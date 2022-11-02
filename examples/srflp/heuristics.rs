use engineering::{StateRanking, WidthHeuristic};

use crate::state::State;

#[derive(Debug, Copy, Clone)]
pub struct SrflpRanking;

impl StateRanking for SrflpRanking {
    type State = State;

    fn compare(&self, sa: &Self::State, sb: &Self::State) -> std::cmp::Ordering {
        sa.depth.cmp(&sb.depth)
    }
}

pub struct SrflpWidth {
    nb_vars: usize,
    factor: usize,
}
impl SrflpWidth {
    pub fn new(nb_vars: usize, factor: usize) -> SrflpWidth {
        SrflpWidth { nb_vars, factor }
    }
}
impl WidthHeuristic<State> for SrflpWidth {
    fn max_width(&self, _state: &State) -> usize {
        self.nb_vars * self.factor
    }
}
