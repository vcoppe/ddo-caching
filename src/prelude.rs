use std::{cmp::Ordering, fmt::Display, str::FromStr, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Variable(pub usize);

impl Variable {
    pub fn id(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub var: Variable,
    pub value: isize,
}

pub trait Problem {
    type State;

    fn nb_variables(&self) -> usize;
    fn initial_state(&self) -> Self::State;
    fn initial_value(&self) -> isize;

    fn next_variable(&self, next_layer: &mut dyn Iterator<Item = &Self::State>)
        -> Option<Variable>;

    fn for_each_in_domain<F>(&self, var: Variable, state: &Self::State, f: F)
    where
        F: FnMut(Decision);

    fn transition(&self, state: &Self::State, decision: Decision) -> Self::State;
    fn transition_cost(&self, state: &Self::State, decision: Decision) -> isize;

    // only useful in order to introduce long arcs (pooled mdd)
    fn impacted_by(&self, _var: Variable, _state: &Self::State) -> bool {
        true
    }
    // rub
    fn estimate(&self, _state: &Self::State) -> isize {
        isize::MAX
    }
}

pub trait Relaxation {
    type State;

    // relaxation
    fn merge(&self, states: &mut dyn Iterator<Item = &Self::State>) -> Self::State;
    fn relax(
        &self,
        source: &Self::State,
        dest: &Self::State,
        new: &Self::State,
        decision: Decision,
        cost: isize,
    ) -> isize;
}

pub trait StateRanking {
    type State;

    // Greater means better -> more likely to be kept
    fn compare(&self, a: &Self::State, b: &Self::State) -> Ordering;
}

pub trait WidthHeuristic<State> {
    // Estimates a good max width for the given state
    fn max_width(&self, state: &State) -> usize;
}

pub trait Solver {
    fn maximize(&mut self);
    fn best_value(&self) -> Option<isize>;
    fn best_solution(&self) -> Option<Vec<Decision>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolutionStatus {
    Proved,
    Interrupted,
}
impl Display for ResolutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolutionStatus::Proved => write!(f, "Proved"),
            ResolutionStatus::Interrupted => write!(f, "Timeout"),
        }
    }
}

pub trait InterruptibleSolver: Solver {
    fn maximize_with_interrupt<I>(&mut self, interrupt: I) -> ResolutionStatus
    where
        I: Fn() -> bool + Send + Sync + 'static;
    //
    fn best_value_so_far(&self) -> Option<isize>;
    fn best_solution_so_far(&self) -> Option<Vec<Decision>>;
    //
    fn best_upper_bound(&self) -> isize;
    fn best_lower_bound(&self) -> isize;
}

pub trait Frontier {
    type State;

    /// This is how you push a node onto the frontier.
    fn push(&mut self, node: SubProblem<Self::State>);
    /// This method yields the most promising node from the frontier.
    /// # Note:
    /// The solvers rely on the assumption that a frontier will pop nodes in
    /// descending upper bound order. Hence, it is a requirement for any fringe
    /// implementation to enforce that requirement.
    fn pop(&mut self) -> Option<SubProblem<Self::State>>;
    /// This method clears the frontier: it removes all nodes from the queue.
    fn clear(&mut self);
    /// Yields the length of the queue.
    fn len(&self) -> usize;
    /// Returns true iff the finge is empty (len == 0)
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/* -------------------------------------------------------------------------- */
/* -------------------------------------------------------------------------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationType {
    Exact,
    Relaxed,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutsetType {
    LastExactLayer,
    Frontier,
}
impl FromStr for CutsetType {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "lel" => Ok(Self::LastExactLayer),
            "frontier" => Ok(Self::Frontier),
            _ => Err("The only supported frontier types are 'parallel' and 'barrier'"),
        }
    }
}
impl Display for CutsetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LastExactLayer => write!(f, "lel"),
            Self::Frontier => write!(f, "frontier"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubProblem<T> {
    pub state: Arc<T>,
    pub value: isize,
    pub path: Vec<Decision>,
    pub ub: isize,
}

pub struct CompilationInput<'a, P, R, O>
where
    P: Problem,
    R: Relaxation<State = P::State>,
    O: StateRanking<State = P::State>,
{
    pub comp_type: CompilationType,
    pub max_width: usize,
    pub problem: &'a P,
    pub relaxation: &'a R,
    pub ranking: &'a O,
    pub residual: SubProblem<P::State>,
    pub best_lb: isize,
}

pub trait DecisionDiagram {
    type State;

    fn compile<P, R, O>(&mut self, input: &CompilationInput<P, R, O>)
    where
        P: Problem<State = Self::State>,
        R: Relaxation<State = P::State>,
        O: StateRanking<State = P::State>;

    fn is_exact(&self) -> bool;
    fn best_value(&self) -> Option<isize>;
    fn best_solution(&self) -> Option<Vec<Decision>>;

    /// FIXME
    /// This can only be called if the dd was compiled in relaxed mode.
    /// WARNING: no check is made to ensure you are using this method right !
    fn drain_cutset<F>(&mut self, func: F)
    where
        F: FnMut(SubProblem<Self::State>);
}

impl FromStr for CompilationType {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "exact" => Ok(CompilationType::Exact),
            "relaxed" => Ok(CompilationType::Relaxed),
            "restricted" => Ok(CompilationType::Restricted),
            _ => Err("Only 'exact', 'relaxed' and 'restricted' are allowed"),
        }
    }
}
impl Display for CompilationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilationType::Exact => write!(f, "exact"),
            CompilationType::Relaxed => write!(f, "relaxed"),
            CompilationType::Restricted => write!(f, "restricted"),
        }
    }
}
