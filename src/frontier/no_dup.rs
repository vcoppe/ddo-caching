use compare::Compare;
use rustc_hash::FxHashMap;
use std::cmp::Ordering;
use std::cmp::Ordering::{Greater, Less};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::{hash::Hash, sync::Arc};

use crate::{Frontier, StateRanking, SubProblem};

use self::Action::{BubbleDown, BubbleUp, DoNothing};

use super::MaxUB;

/// This is a type-safe identifier for some node in the queue.
/// Basically, this NodeId equates to the position of the identified
/// node in the `nodes` list from the `NoDupHeap`.
#[derive(Debug, Copy, Clone)]
struct NodeId(usize);

/// An enum to know what needs to be done with a given node id
#[derive(Debug, Copy, Clone)]
enum Action {
    DoNothing,
    BubbleUp(NodeId),
    BubbleDown(NodeId),
}

/// This is an updatable binary heap backed by a vector which ensures that
/// items remain ordered in the priority queue while guaranteeing that a
/// given state will only ever be present *ONCE* in the priority queue (the
/// node with the longest path to state is the only kept copy).
pub struct NoDupFrontier<'a, O>
where
    O: StateRanking,
    O::State: Eq + Hash + Clone,
{
    /// This is the comparator used to order the nodes in the binary heap
    cmp: MaxUB<'a, O>,
    /// A mapping that associates some state to a node identifier.
    states: FxHashMap<Arc<O::State>, NodeId>,
    /// The actual payload (nodes) ordered in the list
    nodes: Vec<SubProblem<O::State>>,
    /// The position of the items in the heap
    pos: Vec<usize>,
    /// This is the actual heap which orders nodes.
    heap: Vec<NodeId>,
    /// The positions in the `nodes` vector that can be recycled.
    recycle_bin: Vec<NodeId>,
}

impl<'a, O> Frontier for NoDupFrontier<'a, O>
where
    O: StateRanking,
    O::State: Eq + Hash + Clone,
{
    type State = O::State;

    /// Pushes one node onto the heap while ensuring that only one copy of the
    /// node (identified by its state) is kept in the heap.
    ///
    /// # Note:
    /// In the event where the heap already contains a copy `x` of a node having
    /// the same state as the `node` being pushed. The priority of the node
    /// left in the heap might be affected. If `node` node is "better" (greater
    /// UB and or longer longest path), the priority of the node will be
    /// increased. As always, in the event where the newly pushed node has a
    /// longer longest path than the pre-existing node, that one will be kept.
    fn push(&mut self, mut node: SubProblem<O::State>) {
        let state = Arc::clone(&node.state);

        let action = match self.states.entry(state) {
            Occupied(e) => {
                let id = *e.get();

                // info about the pre-existing node
                let old_lp = self.nodes[id.0].value;
                let old_ub = self.nodes[id.0].ub;
                // info about the new node
                let new_lp = node.value;
                let new_ub = node.ub;
                // make sure that ub is the max of the known ubs
                node.ub = new_ub.max(old_ub);

                let action = if self.cmp.compare(&node, &self.nodes[id.0]) == Greater {
                    BubbleUp(id)
                } else {
                    DoNothing
                };

                if new_lp > old_lp {
                    self.nodes[id.0] = node;
                }
                if new_ub > old_ub {
                    self.nodes[id.0].ub = new_ub;
                }

                action
            }
            Vacant(e) => {
                let id = if self.recycle_bin.is_empty() {
                    let id = NodeId(self.nodes.len());
                    self.nodes.push(node);
                    self.pos.push(0); // dummy
                    id
                } else {
                    let id = self.recycle_bin.pop().unwrap();
                    self.nodes[id.0] = node;
                    id
                };

                self.heap.push(id);
                self.pos[id.0] = self.heap.len() - 1;
                e.insert(id);
                BubbleUp(id)
            }
        };

        // restore the invariants
        self.process_action(action);
    }

    /// Pops the best node out of the heap. Here, the best is defined as the
    /// node having the best upper bound, with the longest `lp_len`.
    fn pop(&mut self) -> Option<SubProblem<Self::State>> {
        if self.is_empty() {
            return None;
        }

        let id = self.heap.swap_remove(0);
        let action = if self.heap.is_empty() {
            DoNothing
        } else {
            self.pos[self.heap[0].0] = 0;
            BubbleDown(self.heap[0])
        };

        self.process_action(action);
        self.recycle_bin.push(id);

        let node = self.nodes[id.0].clone();
        self.states.remove(&node.state);

        Some(node)
    }

    /// Clears the content of the heap to reset it to a state equivalent to
    /// a fresh instantiation of the heap.
    fn clear(&mut self) {
        self.states.clear();
        self.nodes.clear();
        self.pos.clear();
        self.heap.clear();
        self.recycle_bin.clear();
    }

    /// Returns the 'length' of the heap. That is, the number of items that
    /// can still be popped out of the heap.
    fn len(&self) -> usize {
        self.heap.len()
    }
}

impl<'a, O> NoDupFrontier<'a, O>
where
    O: StateRanking,
    O::State: Eq + Hash + Clone,
{
    /// Creates a new instance of the no dup heap which uses cmp as
    /// comparison criterion.
    pub fn new(ranking: &'a O) -> Self {
        Self {
            cmp: MaxUB(ranking),
            states: Default::default(),
            nodes: vec![],
            pos: vec![],
            heap: vec![],
            recycle_bin: vec![],
        }
    }

    /// Returns true iff the heap is empty (len() == 0)
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Internal helper method to bubble a node up or down, depending of the
    /// specified action.
    fn process_action(&mut self, action: Action) {
        match action {
            BubbleUp(id) => self.bubble_up(id),
            BubbleDown(id) => self.bubble_down(id),
            DoNothing => { /* sweet life */ }
        }
    }
    /// Internal helper method to return the position of a node in the heap.
    fn position(&self, n: NodeId) -> usize {
        self.pos[n.0]
    }
    /// Internal helper method to compare the nodes identified by the ids found
    /// at the given positions in the heap.
    fn compare_at_pos(&self, x: usize, y: usize) -> Ordering {
        let node_x = &self.nodes[self.heap[x].0];
        let node_y = &self.nodes[self.heap[y].0];
        self.cmp.compare(node_x, node_y)
    }
    /// Internal method to bubble a node up and restore the heap invariant.
    fn bubble_up(&mut self, id: NodeId) {
        let mut me = self.position(id);
        let mut parent = self.parent(me);

        while !self.is_root(me) && self.compare_at_pos(me, parent) == Greater {
            let p_id = self.heap[parent];

            self.pos[p_id.0] = me;
            self.pos[id.0] = parent;
            self.heap[me] = p_id;
            self.heap[parent] = id;

            me = parent;
            parent = self.parent(me);
        }
    }
    /// Internal method to sink a node down so as to restor the heap invariant.
    fn bubble_down(&mut self, id: NodeId) {
        let mut me = self.position(id);
        let mut kid = self.max_child_of(me);

        while kid > 0 && self.compare_at_pos(me, kid) == Less {
            let k_id = self.heap[kid];

            self.pos[k_id.0] = me;
            self.pos[id.0] = kid;
            self.heap[me] = k_id;
            self.heap[kid] = id;

            me = kid;
            kid = self.max_child_of(me);
        }
    }
    /// Internal helper method that returns the position of the node which is
    /// the parent of the node at `pos` in the heap.
    fn parent(&self, pos: usize) -> usize {
        if self.is_root(pos) {
            pos
        } else if self.is_left(pos) {
            pos / 2
        } else {
            pos / 2 - 1
        }
    }
    /// Internal helper method that returns the position of the child of the
    /// node at position `pos` which is considered to be the maximum of the
    /// children of that node.
    ///
    /// # Warning
    /// When the node at `pos` is a leaf, this method returns **0** for the
    /// position of the child. This value 0 acts as a marker to tell that no
    /// child is to be found.
    fn max_child_of(&self, pos: usize) -> usize {
        let size = self.len();
        let left = self.left_child(pos);
        let right = self.right_child(pos);

        if left >= size {
            return 0;
        }
        if right >= size {
            return left;
        }

        match self.compare_at_pos(left, right) {
            Greater => left,
            _ => right,
        }
    }
    /// Internal helper method to return the position of the left child of
    /// the node at the given `pos`.
    fn left_child(&self, pos: usize) -> usize {
        pos * 2 + 1
    }
    /// Internal helper method to return the position of the right child of
    /// the node at the given `pos`.
    fn right_child(&self, pos: usize) -> usize {
        pos * 2 + 2
    }
    /// Internal helper method which returns true iff the node at `pos` is the
    /// root of the binary heap (position is zero).
    fn is_root(&self, pos: usize) -> bool {
        pos == 0
    }
    /// Internal helper method which returns true iff the node at `pos` is the
    /// left child of its parent.
    fn is_left(&self, pos: usize) -> bool {
        pos % 2 == 1
    }
    /*
    /// Internal helper method which returns true iff the node at `pos` is the
    /// right child of its parent.
    fn is_right(&self, pos: usize) -> bool {
        pos % 2 == 0
    }
    */
}
