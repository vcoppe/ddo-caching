use std::{collections::hash_map::Entry, hash::Hash, sync::Arc};

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::{
    prelude::{CompilationInput, CompilationType, Decision, Problem, Relaxation, StateRanking},
    DecisionDiagram, SubProblem, CutsetType,
};

use super::node_flags::NodeFlags;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
struct NodeId(usize);

#[derive(Debug, Clone, Copy)]
struct EdgeId(usize);

#[derive(Debug, Clone)]
struct Node<T> {
    //_my_id   : NodeId,
    state: Arc<T>,
    value: isize,
    best: Option<EdgeId>,
    inbound: Option<EdgeId>,
    //
    depth: usize,
    //
    value_bot: isize,
    theta: isize,
    //
    rub: isize,
    //
    flags: NodeFlags,
}

#[derive(Debug, Clone, Copy)]
struct Edge {
    //_my_id   : EdgeId,
    from: NodeId,
    //to      : NodeId,
    decision: Decision,
    cost: isize,
    next: Option<EdgeId>,
}
#[derive(Debug)]
pub struct BarrierInfo {
    pub theta: isize,
    pub explored: bool,
}

#[derive(Debug, Clone)]
pub struct Barrier<T>
where
    T: Eq + PartialEq + Hash + Clone,
{
    root_pa: Vec<Decision>,
    //
    barriers: Arc<Vec<RwLock<FxHashMap<Arc<T>, BarrierInfo>>>>,
    //
    nodes: Vec<Node<T>>,
    edges: Vec<Edge>,
    //
    prev_l: Vec<NodeId>,
    next_l: FxHashMap<Arc<T>, NodeId>,
    cutset: Vec<NodeId>,
    lel_depth: Option<usize>,
    //
    best_n: Option<NodeId>,
    // ebpo
    exact: bool,
    approximate: bool,
    //
    cutset_type: CutsetType,
    //
    explored: usize,
}
impl<T> DecisionDiagram for Barrier<T>
where
    T: Eq + PartialEq + Hash + Clone,
{
    type State = T;

    fn compile<P, R, O>(&mut self, input: &CompilationInput<P, R, O>)
    where
        P: Problem<State = Self::State>,
        R: Relaxation<State = P::State>,
        O: StateRanking<State = P::State>,
    {
        self._compile(input)
    }

    fn is_exact(&self) -> bool {
        self.exact
    }

    fn best_value(&self) -> Option<isize> {
        self._best_value()
    }

    fn best_solution(&self) -> Option<Vec<Decision>> {
        self._best_solution()
    }

    fn drain_cutset<F>(&mut self, func: F)
    where
        F: FnMut(SubProblem<T>),
    {
        self._drain_cutset(func)
    }
}
impl<T> Barrier<T>
where
    T: Eq + PartialEq + Hash + Clone,
{
    pub fn new(barriers: Arc<Vec<RwLock<FxHashMap<Arc<T>, BarrierInfo>>>>, cutset_type: CutsetType) -> Self {
        Self {
            root_pa: vec![],
            barriers: barriers,
            nodes: vec![],
            edges: vec![],
            prev_l: Default::default(),
            next_l: Default::default(),
            cutset: vec![],
            lel_depth: None,
            best_n: None,
            exact: true,
            approximate: false,
            cutset_type: cutset_type,
            explored: 0,
        }
    }
    fn clear(&mut self) {
        self.root_pa.clear();
        self.nodes.clear();
        self.edges.clear();
        self.next_l.clear();
        self.cutset.clear();
        self.lel_depth = None;
        self.best_n = None;
        self.exact = true;
        self.approximate = false;
        self.explored = 0;
    }

    fn _is_exact(&self, comp_type: CompilationType) -> bool {
        !self.approximate
            || (comp_type == CompilationType::Relaxed && self.has_exact_best_path(self.best_n))
    }

    fn has_exact_best_path(&self, node: Option<NodeId>) -> bool {
        if let Some(node_id) = node {
            let n = &self.nodes[node_id.0];
            if n.flags.is_exact() {
                true
            } else {
                !n.flags.is_relaxed()
                    && self.has_exact_best_path(n.best.map(|e| self.edges[e.0].from))
            }
        } else {
            true
        }
    }

    fn _best_value(&self) -> Option<isize> {
        self.best_n.map(|id| self.nodes[id.0].value)
    }

    fn _best_solution(&self) -> Option<Vec<Decision>> {
        self.best_n.map(|id| self._best_path(id))
    }

    fn _best_path(&self, id: NodeId) -> Vec<Decision> {
        Self::_best_path_partial_borrow(id, &self.root_pa, &self.nodes, &self.edges)
    }

    fn _best_path_partial_borrow(
        id: NodeId,
        root_pa: &[Decision],
        nodes: &[Node<T>],
        edges: &[Edge],
    ) -> Vec<Decision> {
        let mut sol = root_pa.to_owned();
        let mut edge_id = nodes[id.0].best;
        while let Some(eid) = edge_id {
            let edge = edges[eid.0];
            sol.push(edge.decision);
            edge_id = nodes[edge.from.0].best;
        }
        sol
    }

    fn _drain_cutset<F>(&mut self, mut func: F)
    where
        F: FnMut(SubProblem<T>),
    {
        if let Some(best_value) = self.best_value() {
            for node_id in self.cutset.drain(..) {
                let node = &self.nodes[node_id.0];

                if node.flags.is_marked() {
                    let rub = node.value.saturating_add(node.rub);
                    let locb = node.value.saturating_add(node.value_bot);
                    let ub = rub.min(locb).min(best_value);

                    func(SubProblem {
                        state: node.state.clone(),
                        value: node.value,
                        path: Self::_best_path_partial_borrow(
                            node_id,
                            &self.root_pa,
                            &self.nodes,
                            &self.edges,
                        ),
                        ub,
                    })
                }
            }
        }
    }

    pub fn get_explored(&self) -> usize {
        self.explored
    }

    fn _compile<P, R, O>(&mut self, input: &CompilationInput<P, R, O>)
    where
        P: Problem<State = T>,
        R: Relaxation<State = P::State>,
        O: StateRanking<State = P::State>,
    {
        self.clear();

        let mut curr_l = vec![];

        input
            .residual
            .path
            .iter()
            .copied()
            .for_each(|x| self.root_pa.push(x));

        let root_depth = self.root_pa.len();

        let root_s = input.residual.state.clone();
        let root_v = input.residual.value;
        let root_n = Node {
            state: root_s.clone(),
            value: root_v,
            best: None,
            inbound: None,
            depth: root_depth,
            value_bot: isize::MIN,
            theta: isize::MAX,
            rub: input.residual.ub - root_v,
            flags: NodeFlags::new_exact(),
        };

        self.nodes.push(root_n);
        self.next_l.insert(root_s, NodeId(0));

        let mut depth = root_depth;

        while let Some(var) = input.problem.next_variable(&mut self.next_l.keys().map(|s| s.as_ref())) {
            self.prev_l.clear();
            for node_id in curr_l.drain(..) {
                self.prev_l.push(node_id);
            }
            for (_, node_id) in self.next_l.drain() {
                curr_l.push(node_id);
            }

            if curr_l.is_empty() {
                return;
            }

            if depth > root_depth && !self.barriers[depth].read().is_empty() {
                // try to prune nodes before expanding them
                curr_l.retain_mut(|node_id| {
                    if self.nodes[node_id.0].flags.is_relaxed() {
                        true
                    } else {
                        let state = &self.nodes[node_id.0].state;
                        let theta = self.barriers[depth].read().get(state).map_or(isize::MIN, |bi| bi.theta);

                        if self.nodes[node_id.0].value > theta {
                            true
                        } else {
                            self.nodes[node_id.0].theta = theta; // set theta for later propagation
                            self.nodes[node_id.0].flags.set_pruned_by_barrier(true);
                            false
                        }
                    }
                });
            }

            match input.comp_type {
                CompilationType::Exact => { /* do nothing: you want to explore the complete DD */ }
                CompilationType::Restricted => {
                    if curr_l.len() > input.max_width {
                        self.restrict(input, &mut curr_l)
                    }
                }
                CompilationType::Relaxed => {
                    if curr_l.len() > input.max_width && depth > root_depth + 1 {
                        self.relax(input, &mut curr_l)
                    }
                }
            }

            for node_id in curr_l.iter() {
                let state = self.nodes[node_id.0].state.clone();
                let rub = input.problem.estimate(state.as_ref());
                self.nodes[node_id.0].rub = rub;
                let ub = rub.saturating_add(self.nodes[node_id.0].value);

                if ub > input.best_lb {
                    input.problem.for_each_in_domain(var, state.as_ref(), |decision| {
                        self.branch_on(*node_id, decision, input.problem)
                    });

                    self.explored += 1;

                    if matches!(input.comp_type, CompilationType::Relaxed) && self.nodes[node_id.0].flags.is_exact() {
                        // if we made it to here, we have improved the threshold
                        // try to update threshold for other threads already
                        self.try_update_barrier(depth, state, self.nodes[node_id.0].value, false);
                    }
                } else {
                    self.nodes[node_id.0].theta = input.best_lb.saturating_sub(rub); // set theta for later propagation

                    if matches!(input.comp_type, CompilationType::Relaxed) && self.nodes[node_id.0].flags.is_exact() {
                        // try to update threshold for other threads already
                        self.try_update_barrier(depth, state, self.nodes[node_id.0].theta, false);
                    }
                }
            }

            depth += 1;
        }

        //
        self.best_n = self
            .next_l
            .values()
            .copied()
            .max_by_key(|id| self.nodes[id.0].value);
        self.exact = self._is_exact(input.comp_type);
        //
        if matches!(input.comp_type, CompilationType::Relaxed) {
            self.compute_local_bounds_and_theta(input.best_lb);
        }
    }

    fn branch_on<P: Problem<State = T>>(
        &mut self,
        from_id: NodeId,
        decision: Decision,
        problem: &P,
    ) {
        let state = self.nodes[from_id.0].state.as_ref();
        let next_state = Arc::new(problem.transition(state, decision));
        let cost = problem.transition_cost(state, decision);

        match self.next_l.entry(next_state.clone()) {
            Entry::Vacant(e) => {
                let node_id = NodeId(self.nodes.len());
                let edge_id = EdgeId(self.edges.len());

                self.edges.push(Edge {
                    //my_id: edge_id,
                    from: from_id,
                    //to   : node_id,
                    decision,
                    cost,
                    next: None,
                });
                self.nodes.push(Node {
                    //my_id  : node_id,
                    state: next_state,
                    value: self.nodes[from_id.0].value.saturating_add(cost),
                    best: Some(edge_id),
                    inbound: Some(edge_id),
                    //
                    depth: self.nodes[from_id.0].depth + 1,
                    //
                    value_bot: isize::MIN,
                    theta: isize::MAX,
                    //
                    rub: isize::MAX,
                    flags: self.nodes[from_id.0].flags,
                });

                e.insert(node_id);
            }
            Entry::Occupied(e) => {
                let node_id = *e.get();
                let flags = self.nodes[from_id.0].flags;
                let value = self.nodes[from_id.0].value.saturating_add(cost);
                let node = &mut self.nodes[node_id.0];

                let edge_id = EdgeId(self.edges.len());
                self.edges.push(Edge {
                    //my_id: edge_id,
                    from: from_id,
                    //to   : node_id,
                    decision,
                    cost,
                    next: node.inbound,
                });

                node.inbound = Some(edge_id);
                if value > node.value || (value == node.value && flags.is_exact()) {
                    node.value = value;
                    node.best = Some(edge_id);
                    node.flags = flags;
                }
            }
        }
    }

    fn restrict<P, R, O>(
        &mut self,
        input: &CompilationInput<P, R, O>,
        curr_l: &mut Vec<NodeId>,
    ) where
        P: Problem<State = T>,
        R: Relaxation<State = P::State>,
        O: StateRanking<State = P::State>,
    {
        self.approximate = true;
        curr_l.sort_unstable_by(|a, b| {
            self.nodes[a.0]
                .value
                .cmp(&self.nodes[b.0].value)
                .then_with(|| input.ranking.compare(self.nodes[a.0].state.as_ref(), self.nodes[b.0].state.as_ref()))
                .reverse()
        }); // reverse because greater means more likely to be kept
        curr_l.truncate(input.max_width);
    }

    fn relax<P, R, O>(&mut self, input: &CompilationInput<P, R, O>, curr_l: &mut Vec<NodeId>)
    where
        P: Problem<State = T>,
        R: Relaxation<State = P::State>,
        O: StateRanking<State = P::State>,
    {
        if self.cutset_type == CutsetType::LastExactLayer && !self.approximate {
            for id in self.prev_l.iter() {
                self.cutset.push(*id);
                self.nodes[id.0].flags.set_cutset(true);
                self.lel_depth = Some(self.nodes[id.0].depth);
            }
        }

        self.approximate = true;
        curr_l.sort_unstable_by(|a, b| {
            self.nodes[a.0]
                .value
                .cmp(&self.nodes[b.0].value)
                .then_with(|| input.ranking.compare(self.nodes[a.0].state.as_ref(), self.nodes[b.0].state.as_ref()))
                .reverse()
        }); // reverse because greater means more likely to be kept

        //--
        let (keep, merge) = curr_l.split_at_mut(input.max_width - 1);
        let merged = Arc::new(input.relaxation.merge(&mut merge.iter().map(|node_id| self.nodes[node_id.0].state.as_ref())));

        let recycled = keep.iter().find(|node_id| self.nodes[node_id.0].state.eq(&merged)).map(|node_id| *node_id);

        let merged_id = recycled.unwrap_or_else(|| {
            let node_id = NodeId(self.nodes.len());
            self.nodes.push(Node {
                //my_id  : node_id,
                state: merged.clone(),
                value: isize::MIN,
                best: None,    // yet
                inbound: None, // yet
                //
                depth: self.nodes[merge[0].0].depth,
                //
                value_bot: isize::MIN,
                theta: isize::MAX,
                //
                rub: isize::MAX,
                flags: NodeFlags::new_relaxed(),
            });
            node_id
        });

        self.nodes[merged_id.0].flags.set_relaxed(true);

        for drop_id in merge {
            self.nodes[drop_id.0].flags.set_deleted(true);

            let mut edge_id = self.nodes[drop_id.0].inbound;
            while let Some(eid) = edge_id {
                let edge = self.edges[eid.0];
                let src = self.nodes[edge.from.0].state.as_ref();

                let rcost = input
                    .relaxation
                    .relax(src, self.nodes[drop_id.0].state.as_ref(), merged.as_ref(), edge.decision, edge.cost);

                let new_eid = EdgeId(self.edges.len());
                let new_edge = Edge {
                    //my_id: new_eid,
                    from: edge.from,
                    //to   : merged_id,
                    decision: edge.decision,
                    cost: rcost,
                    next: self.nodes[merged_id.0].inbound,
                };
                self.edges.push(new_edge);
                self.nodes[merged_id.0].inbound = Some(new_eid);

                let new_value = self.nodes[edge.from.0].value.saturating_add(rcost);
                if new_value >= self.nodes[merged_id.0].value {
                    self.nodes[merged_id.0].best = Some(new_eid);
                    self.nodes[merged_id.0].value = new_value;
                }

                edge_id = edge.next;
            }
        }

        if recycled.is_some() {
            curr_l.truncate(input.max_width);
            let saved_id = curr_l[input.max_width - 1];
            self.nodes[saved_id.0].flags.set_deleted(false);
        } else {
            curr_l.truncate(input.max_width - 1);
            curr_l.push(merged_id);
        }
    }

    fn compute_local_bounds_and_theta(&mut self, best_lb: isize) {
        for node_id in self.next_l.values() {
            // init for local bounds
            self.nodes[node_id.0].value_bot = 0;
            self.nodes[node_id.0].flags.set_marked(true);

            if self.cutset_type == CutsetType::LastExactLayer && !self.approximate {
                self.nodes[node_id.0].flags.set_cutset(true);
            } else if self.cutset_type == CutsetType::Frontier && self.nodes[node_id.0].flags.is_exact() {
                self.nodes[node_id.0].flags.set_cutset(true);
            }
        }

        // propagate values upwards and update barrier
        for node_id in (0..self.nodes.len()).rev() {
            let node_id = NodeId(node_id);

            if self.nodes[node_id.0].flags.is_deleted() {
                continue;
            }

            if self.nodes[node_id.0].flags.is_cutset() {
                // set theta for frontier nodes
                let locb = self.nodes[node_id.0].value.saturating_add(self.nodes[node_id.0].value_bot);
                if locb < best_lb {
                    let pruning_theta = best_lb.saturating_sub(self.nodes[node_id.0].value_bot);
                    self.nodes[node_id.0].theta = self.nodes[node_id.0].theta.min(pruning_theta);
                } else {
                    self.nodes[node_id.0].theta = self.nodes[node_id.0].theta.min(self.nodes[node_id.0].value);
                }
            }

            if self.nodes[node_id.0].flags.is_exact() &&
                !self.nodes[node_id.0].flags.is_pruned_by_barrier() // theta was not improved in this case
            {
                // fill barrier
                self.try_update_barrier(
                    self.nodes[node_id.0].depth, 
                    self.nodes[node_id.0].state.clone(), 
                    self.nodes[node_id.0].theta, 
                    !self.nodes[node_id.0].flags.is_cutset() // do not mark nodes of the frontier cutset as explored
                );
            }

            let mut inbound = self.nodes[node_id.0].inbound;
            while let Some(edge_id) = inbound {
                let edge = self.edges[edge_id.0];

                // propagate for local bounds
                if self.nodes[node_id.0].flags.is_marked() {
                    let lp_from_bot_using_edge = self.nodes[node_id.0].value_bot.saturating_add(edge.cost);

                    self.nodes[edge.from.0].value_bot = self.nodes[edge.from.0]
                        .value_bot
                        .max(lp_from_bot_using_edge);
                    
                    self.nodes[edge.from.0].flags.set_marked(true);
                }

                // propagate for theta
                let theta_using_edge = self.nodes[node_id.0].theta.saturating_sub(edge.cost);
                self.nodes[edge.from.0].theta = self.nodes[edge.from.0].theta.min(theta_using_edge);

                if self.cutset_type == CutsetType::Frontier && self.nodes[node_id.0].flags.is_marked(){
                    if !self.nodes[node_id.0].flags.is_exact() && self.nodes[edge.from.0].flags.is_exact() &&
                        !self.nodes[edge.from.0].flags.is_cutset() {
                        self.nodes[edge.from.0].flags.set_cutset(true);
                        self.cutset.push(edge.from);
                    }
                }

                inbound = edge.next;
            }
        }
    }

    fn try_update_barrier(&mut self, depth: usize, state: Arc<T>, theta: isize, explored: bool)
    {
        // do not store thresholds below last exact layer, otherwise it blocks transitions below the cutset nodes
        if self.cutset_type == CutsetType::LastExactLayer && self.lel_depth.is_some() && depth > self.lel_depth.unwrap() {
            return;
        }

        let update = self.barriers[depth].read().get(&state).map_or(true, |info| {
            if theta > info.theta || (theta == info.theta && !info.explored && explored) {
                true
            } else {
                false
            }
        });

        if update {
            self.barriers[depth].write().insert(state, BarrierInfo { theta, explored });
        }
    }
}
