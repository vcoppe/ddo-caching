//! This module provides the implementation of usual frontiers.
use compare::Compare;
use std::cmp::Ordering;

use crate::{StateRanking, SubProblem};

#[derive(Debug, Clone, Copy)]
struct MaxUB<'a, O: StateRanking>(pub &'a O);
impl<O: StateRanking> Compare<SubProblem<O::State>> for MaxUB<'_, O> {
    fn compare(&self, l: &SubProblem<O::State>, r: &SubProblem<O::State>) -> Ordering {
        l.ub.cmp(&r.ub)
            .then_with(|| self.0.compare(&l.state, &r.state))
    }
}

pub mod no_dup;
pub mod simple;

pub use no_dup::*;
pub use simple::*;
