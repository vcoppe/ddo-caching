use crate::{Frontier, StateRanking, SubProblem};
use binary_heap_plus::BinaryHeap;

use super::MaxUB;

pub struct SimpleFrontier<'a, O: StateRanking> {
    heap: BinaryHeap<SubProblem<O::State>, MaxUB<'a, O>>,
}
impl<'a, O: StateRanking> SimpleFrontier<'a, O> {
    pub fn new(ranking: &'a O) -> Self {
        Self {
            heap: BinaryHeap::from_vec_cmp(vec![], MaxUB(ranking)),
        }
    }
}
impl<O: StateRanking> Frontier for SimpleFrontier<'_, O> {
    type State = O::State;

    fn push(&mut self, node: SubProblem<O::State>) {
        self.heap.push(node)
    }

    fn pop(&mut self) -> Option<SubProblem<O::State>> {
        self.heap.pop()
    }

    fn clear(&mut self) {
        self.heap.clear()
    }

    fn len(&self) -> usize {
        self.heap.len()
    }
}
