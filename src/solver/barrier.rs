use std::{sync::Arc, hash::Hash};

use parking_lot::{Condvar, Mutex, RwLock};
use rustc_hash::FxHashMap;

use crate::{
    CompilationInput, CompilationType, Decision, DecisionDiagram, Frontier, InterruptibleSolver,
    Problem, Relaxation, ResolutionStatus, Solver, StateRanking, SubProblem, WidthHeuristic, NoDupFrontier, Barrier, BarrierInfo, CutsetType,
};

/// The shared data that may only be manipulated within critical sections
struct Critical<'a, O>
where
    O: StateRanking,
    O::State: Eq + PartialEq + Hash + Clone,
{
    /// This is the fringe: the set of nodes that must still be explored before
    /// the problem can be considered 'solved'.
    ///
    /// # Note:
    /// This fringe orders the nodes by upper bound (so the highest ub is going
    /// to pop first). So, it is guaranteed that the upper bound of the first
    /// node being popped is an upper bound on the value reachable by exploring
    /// any of the nodes remaining on the fringe. As a consequence, the
    /// exploration can be stopped as soon as a node with an ub <= current best
    /// lower bound is popped.
    fringe: NoDupFrontier<'a, O>,
    /// This is the number of nodes that are currently being explored.
    ///
    /// # Note
    /// This information may seem innocuous/superfluous, whereas in fact it is
    /// very important. Indeed, this is the piece of information that lets us
    /// distinguish between a node-starvation and the completion of the problem
    /// resolution. The bottom line is, this counter needs to be carefully
    /// managed to guarantee the termination of all threads.
    ongoing: usize,
    /// This is a counter that tracks the number of nodes that have effectively
    /// been explored. That is, the number of nodes that have been popped from
    /// the fringe, and for which a restricted and relaxed mdd have been developed.
    explored: usize,
    explored_dd: usize,
    /// This is a counter of the number of nodes in the fringe, for each level of the model
    open_by_layer: Vec<usize>,
    /// This is a counter of the number of nodes in ongoing expansion, for each level of the model
    ongoing_by_layer: Vec<usize>,
    /// This is the index of the lowest level above which there are no nodes in the fringe
    lowest_active_layer: usize,
    /// This is the value of the best known lower bound.
    best_lb: isize,
    /// This is the value of the best known lower bound.
    /// *WARNING* This one only gets set when the interrupt condition is satisfied
    best_ub: isize,
    /// If set, this keeps the info about the best solution so far.
    best_sol: Option<Vec<Decision>>,
    /// This vector is used to store the upper bound on the node which is
    /// currently processed by each thread.
    ///
    /// # Note
    /// When a thread is idle (or more generally when it is done with processing
    /// it node), it should place the value i32::min_value() in its corresponding
    /// cell.
    upper_bounds: Vec<isize>,
    interrupted: bool,
}
/// The state which is shared among the many running threads: it provides an
/// access to the critical data (protected by a mutex) as well as a monitor
/// (condvar) to park threads in case of node-starvation.
struct Shared<'a, P, R, O, W>
where
    P: Problem + Send + Sync + 'a,
    P::State: Eq + PartialEq + Hash + Clone,
    R: Relaxation<State = P::State> + Send + Sync + 'a,
    O: StateRanking<State = P::State> + Send + Sync + 'a,
    W: WidthHeuristic<P::State> + Send + Sync + 'a,
{
    problem: &'a P,
    relaxation: &'a R,
    ranking: &'a O,
    width_heu: &'a W,
    cutset_type: CutsetType,

    /// This is the shared state data which can only be accessed within critical
    /// sections. Therefore, it is protected by a mutex which prevents concurrent
    /// reads/writes.
    critical: Mutex<Critical<'a, O>>,
    barriers: Arc<Vec<RwLock<FxHashMap<Arc<P::State>, BarrierInfo>>>>,
    /// This is the monitor on which nodes must wait when facing an empty fringe.
    /// The corollary, it that whenever a node has completed the processing of
    /// a subproblem, it must wakeup all parked threads waiting on this monitor.
    monitor: Condvar,
}
/// The workload a thread can get from the shared state
enum WorkLoad<T> {
    /// There is no work left to be done: you can safely terminate
    Complete,
    /// The work must stop because of an external cutoff
    Interruption,
    /// There is nothing you can do right now. Check again when you wake up
    Starvation,
    /// The item to process
    WorkItem { node: SubProblem<T> },
}

pub struct BarrierParallelSolver<'a, P, R, O, W>
where
    P: Problem + Send + Sync + 'a,
    P::State: Eq + PartialEq + Hash + Clone,
    R: Relaxation<State = P::State> + Send + Sync + 'a,
    O: StateRanking<State = P::State> + Send + Sync + 'a,
    W: WidthHeuristic<P::State> + Send + Sync + 'a,
{
    /// This is the shared state. Each thread is going to take a reference to it.
    shared: Shared<'a, P, R, O, W>,
    /// This is a configuration parameter that tunes the number of threads that
    /// will be spawned to solve the problem. By default, this number amounts
    /// to the number of hardware threads available on the machine.
    nb_threads: usize,
}

// private interface.
impl <'a, P, R, O, W> BarrierParallelSolver<'a, P, R, O, W> 
where 
    P: Problem + Send + Sync + 'a,
    R: Relaxation<State = P::State> + Send + Sync + 'a,
    O: StateRanking<State = P::State> + Send + Sync + 'a,
    W: WidthHeuristic<P::State> + Send + Sync + 'a,
    P::State: Eq + Hash + Clone
{
    pub fn new(
        problem: &'a P,
        relaxation: &'a R,
        ranking: &'a O,
        width: &'a W,
        cutset_type: CutsetType,
    ) -> Self {
        Self::custom(problem, relaxation, ranking, width, cutset_type, num_cpus::get())
    }
}


impl<'a, P, R, O, W> BarrierParallelSolver<'a, P, R, O, W>
where
    P: Problem + Send + Sync + 'a,
    P::State: Eq + PartialEq + Hash + Clone,
    R: Relaxation<State = P::State> + Send + Sync + 'a,
    O: StateRanking<State = P::State> + Send + Sync + 'a,
    W: WidthHeuristic<P::State> + Send + Sync + 'a,
{
    pub fn custom(
        problem: &'a P,
        relaxation: &'a R,
        ranking: &'a O,
        width_heu: &'a W,
        cutset_type: CutsetType,
        nb_threads: usize,
    ) -> Self {
        let mut barriers = vec![];
        for _ in 0..=problem.nb_variables() {
            barriers.push(RwLock::new(Default::default()));
        }
        let barriers = Arc::new(barriers);
        BarrierParallelSolver {
            shared: Shared {
                problem,
                relaxation,
                ranking,
                width_heu,
                cutset_type: cutset_type,
                //
                monitor: Condvar::new(),
                critical: Mutex::new(Critical {
                    best_sol: None,
                    best_lb: isize::MIN,
                    best_ub: isize::MAX,
                    upper_bounds: vec![isize::MAX; nb_threads],
                    fringe: NoDupFrontier::new(ranking),
                    ongoing: 0,
                    explored: 0,
                    explored_dd: 0,
                    open_by_layer: vec![0; problem.nb_variables()+1],
                    ongoing_by_layer: vec![0; problem.nb_variables()+1],
                    lowest_active_layer: 0,
                    interrupted: false,
                }),
                barriers: barriers
            },
            nb_threads,
        }
    }
    /// Sets the number of threads used by the solver
    pub fn with_nb_threads(mut self, nb_threads: usize) -> Self {
        self.nb_threads = nb_threads;
        self
    }

    /// This method initializes the problem resolution. Put more simply, this
    /// method posts the root node of the mdd onto the fringe so that a thread
    /// can pick it up and the processing can be bootstrapped.
    fn initialize(&self) {
        let root = self.root_node();
        let mut critical = self.shared.critical.lock();
        critical.fringe.push(root);
        critical.open_by_layer[0] += 1;
    }

    fn root_node(&self) -> SubProblem<P::State> {
        let shared = &self.shared;
        SubProblem {
            state: Arc::new(shared.problem.initial_state()),
            value: shared.problem.initial_value(),
            path: vec![],
            ub: isize::MAX,
        }
    }

    /// This method processes the given `node`. To do so, it reads the current
    /// best lower bound from the critical data. Then it expands a restricted
    /// and possibly a relaxed mdd rooted in `node`. If that is necessary,
    /// it stores cutset nodes onto the fringe for further parallel processing.
    fn process_one_node(
        mdd: &mut Barrier<P::State>,
        shared: &Shared<P, R, O, W>,
        node: SubProblem<P::State>,
    ) -> usize 
    {
        let mut explored_dd = 0;

        // 1. RESTRICTION
        let node_ub = node.ub;
        let best_lb = Self::best_lb(shared);

        if node_ub <= best_lb {
            return explored_dd;
        }

        let width = shared.width_heu.max_width(&node.state);
        let mut compilation = CompilationInput {
            comp_type: CompilationType::Restricted,
            max_width: width,
            problem: shared.problem,
            relaxation: shared.relaxation,
            ranking: shared.ranking,
            residual: node,
            //
            best_lb,
        };

        mdd.compile(&compilation);
        explored_dd += mdd.get_explored();
        Self::maybe_update_best(mdd, shared);
        if mdd.is_exact() {
            return explored_dd;
        }

        // 2. RELAXATION
        let best_lb = Self::best_lb(shared);
        compilation.comp_type = CompilationType::Relaxed;
        compilation.best_lb = best_lb;
        mdd.compile(&compilation);
        explored_dd += mdd.get_explored();
        if mdd.is_exact() {
            Self::maybe_update_best(mdd, shared);
        } else {
            Self::enqueue_cutset(mdd, shared, node_ub);
        }

        return explored_dd;
    }

    fn best_lb(shared: &Shared<P, R, O, W>) -> isize {
        shared.critical.lock().best_lb
    }

    /// This private method updates the shared best known node and lower bound in
    /// case the best value of the current `mdd` expansion improves the current
    /// bounds.
    fn maybe_update_best(mdd: &Barrier<P::State>, shared: &Shared<P, R, O, W>) {
        let mut shared = shared.critical.lock();
        let dd_best_value = mdd.best_value().unwrap_or(isize::MIN);
        if dd_best_value > shared.best_lb {
            shared.best_lb = dd_best_value;
            shared.best_sol = mdd.best_solution();
        }
    }
    /// If necessary, thightens the bound of nodes in the cutset of `mdd` and
    /// then add the relevant nodes to the shared fringe.
    fn enqueue_cutset(mdd: &mut Barrier<P::State>, shared: &Shared<P, R, O, W>, ub: isize) {
        let mut critical = shared.critical.lock();
        let best_lb = critical.best_lb;

        mdd.drain_cutset(|mut cutset_node| {
            cutset_node.ub = ub.min(cutset_node.ub);
            if cutset_node.ub > best_lb {
                let depth = cutset_node.path.len();
                critical.fringe.push(cutset_node);
                critical.open_by_layer[depth] += 1;
            }
        });
    }
    /// Acknowledges that a thread finished processing its node.
    fn notify_node_finished(shared: &Shared<P, R, O, W>, thread_id: usize, depth: usize, explored_dd: usize) {
        let mut critical = shared.critical.lock();
        critical.ongoing -= 1;
        critical.upper_bounds[thread_id] = isize::MAX;
        critical.ongoing_by_layer[depth] -= 1;
        critical.explored_dd += explored_dd;

        shared.monitor.notify_all();
    }

    /// Consults the shared state to fetch a workload. Depending on the current
    /// state, the workload can either be:
    ///
    ///   + Complete, when the problem is solved and all threads should stop
    ///   + Starvation, when there is no subproblem available for processing
    ///     at the time being (but some subproblem are still being processed
    ///     and thus the problem cannot be considered solved).
    ///   + WorkItem, when the thread successfully obtained a subproblem to
    ///     process.
    fn get_workload<I>(
        shared: &Shared<P, R, O, W>,
        thread_id: usize,
        interrupt: I,
    ) -> WorkLoad<P::State>
    where
        I: Fn() -> bool,
    {
        let mut critical = shared.critical.lock();

        // Can we clean up the barrier?
        while critical.lowest_active_layer < shared.problem.nb_variables() &&
                critical.open_by_layer[critical.lowest_active_layer] + critical.ongoing_by_layer[critical.lowest_active_layer] == 0 {
            shared.barriers[critical.lowest_active_layer].write().clear();
            critical.lowest_active_layer += 1;
        }

        // Are we done ?
        if critical.ongoing == 0 && critical.fringe.is_empty() {
            critical.best_ub = critical.best_lb;
            return WorkLoad::Complete;
        }

        // Do we need to stop
        if critical.interrupted {
            return WorkLoad::Interruption;
        } else if interrupt() {
            critical.interrupted = true;

            critical.best_ub = if critical.ongoing > 0 {
                critical
                    .upper_bounds
                    .iter()
                    .copied()
                    .filter(|x| *x != isize::MAX)
                    .max()
                    .unwrap_or(isize::MAX)
            } else {
                let nn = critical.fringe.pop().unwrap();
                nn.ub
            };

            critical.fringe.clear();
            return WorkLoad::Interruption;
        }

        // Nothing to do yet ? => Wait for someone to post jobs
        if critical.fringe.is_empty() {
            shared.monitor.wait(&mut critical);
            return WorkLoad::Starvation;
        }
        // Nothing relevant ? =>  Wait for someone to post jobs
        let mut nn = critical.fringe.pop().unwrap();
        loop {
            if nn.ub <= critical.best_lb {
                critical.fringe.clear();
                critical.open_by_layer.iter_mut().for_each(|o| *o = 0);
                return WorkLoad::Starvation;
            }

            let depth = nn.path.len();

            let explore = shared.barriers[depth].read().get(&nn.state).map_or(true, |info| {
                if nn.value > info.theta || (nn.value == info.theta && !info.explored) {
                    true
                } else {
                    critical.open_by_layer[depth] -= 1;
                    false
                }
            });

            if explore {
                shared.barriers[depth].write().insert(nn.state.clone(), BarrierInfo {theta: nn.value, explored: true});
                break;
            }

            if critical.fringe.is_empty() {
                return WorkLoad::Starvation;
            }

            nn = critical.fringe.pop().unwrap();
        }

        // Consume the current node and process it
        critical.ongoing += 1;
        critical.explored += 1;
        critical.upper_bounds[thread_id] = nn.ub;

        let depth = nn.path.len();
        critical.open_by_layer[depth] -= 1;
        critical.ongoing_by_layer[depth] += 1;

        WorkLoad::WorkItem { node: nn }
    }

    pub fn get_explored(&self) -> usize {
        return self.shared.critical.lock().explored;
    }

    pub fn get_explored_dd(&self) -> usize {
        return self.shared.critical.lock().explored_dd;
    }
}

impl<'a, P, R, O, W> Solver for BarrierParallelSolver<'a, P, R, O, W>
where
    P: Problem + Send + Sync + 'a,
    P::State: Eq + PartialEq + Hash + Clone + Send + Sync,
    R: Relaxation<State = P::State> + Send + Sync + 'a,
    O: StateRanking<State = P::State> + Send + Sync + 'a,
    W: WidthHeuristic<P::State> + Send + Sync + 'a,
{
    /// Applies the branch and bound algorithm proposed by Bergman et al. to
    /// solve the problem to optimality. To do so, it spawns `nb_threads` workers
    /// (long running threads); each of which will continually get a workload
    /// and process it until the problem is solved.
    fn maximize(&mut self) {
        self.initialize();

        std::thread::scope(|s| {
            for i in 0..self.nb_threads {
                let shared = &self.shared;
                s.spawn(move || {
                    let mut mdd = Barrier::<P::State>::new(shared.barriers.clone(), shared.cutset_type);
                    loop {
                        match Self::get_workload(shared, i, || false) {
                            WorkLoad::Complete => break,
                            WorkLoad::Interruption => break, // this one cannot occur
                            WorkLoad::Starvation => continue,
                            WorkLoad::WorkItem { node } => {
                                let depth = node.path.len();
                                let explored_dd = Self::process_one_node(&mut mdd, shared, node);
                                Self::notify_node_finished(shared, i, depth, explored_dd);
                            }
                        }
                    }
                });
            }
        });
    }

    /// Returns the best solution that has been identified for this problem.
    fn best_solution(&self) -> Option<Vec<Decision>> {
        self.shared.critical.lock().best_sol.clone()
    }
    /// Returns the value of the best solution that has been identified for
    /// this problem.
    fn best_value(&self) -> Option<isize> {
        let critical = self.shared.critical.lock();
        critical.best_sol.as_ref().map(|_sol| critical.best_lb)
    }
}

impl<'a, P, R, O, W> InterruptibleSolver for BarrierParallelSolver<'a, P, R, O, W>
where
    P: Problem + Send + Sync + 'a,
    P::State: Eq + PartialEq + Hash + Clone + Send + Sync,
    R: Relaxation<State = P::State> + Send + Sync + 'a,
    O: StateRanking<State = P::State> + Send + Sync + 'a,
    W: WidthHeuristic<P::State> + Send + Sync + 'a,
{
    fn maximize_with_interrupt<I>(&mut self, interrupt: I) -> crate::ResolutionStatus
    where
        I: Fn() -> bool + Send + Sync + 'static,
    {
        self.initialize();
        let callback = &interrupt;
        std::thread::scope(|s| {
            for i in 0..self.nb_threads {
                let shared = &self.shared;
                s.spawn(move || {
                    let mut mdd = Barrier::<P::State>::new(shared.barriers.clone(), shared.cutset_type);
                    loop {
                        match Self::get_workload(shared, i, callback) {
                            WorkLoad::Complete => break,
                            WorkLoad::Interruption => break, // this one cannot occur
                            WorkLoad::Starvation => continue,
                            WorkLoad::WorkItem { node } => {
                                let depth = node.path.len();
                                let explored_dd = Self::process_one_node(&mut mdd, shared, node);
                                Self::notify_node_finished(shared, i, depth, explored_dd);
                            }
                        }
                    }
                });
            }
        });

        let lock = self.shared.critical.lock();
        if !lock.interrupted {
            ResolutionStatus::Proved
        } else {
            ResolutionStatus::Interrupted
        }
    }

    fn best_value_so_far(&self) -> Option<isize> {
        self.best_value()
    }

    fn best_solution_so_far(&self) -> Option<Vec<Decision>> {
        self.best_solution()
    }

    fn best_lower_bound(&self) -> isize {
        self.shared.critical.lock().best_lb
    }

    fn best_upper_bound(&self) -> isize {
        self.shared.critical.lock().best_ub
    }
}
