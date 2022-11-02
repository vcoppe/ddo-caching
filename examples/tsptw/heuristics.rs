use engineering::{StateRanking, WidthHeuristic};

use crate::state::State;

#[derive(Debug, Copy, Clone)]
pub struct TsptwRanking;

impl StateRanking for TsptwRanking {
    type State = State;

    fn compare(&self, sa: &Self::State, sb: &Self::State) -> std::cmp::Ordering {
        sa.depth.cmp(&sb.depth)
    }
}

pub struct TsptwWidth {
    nb_vars: usize,
    factor: usize,
}
impl TsptwWidth {
    pub fn new(nb_vars: usize, factor: usize) -> TsptwWidth {
        TsptwWidth { nb_vars, factor }
    }
}
impl WidthHeuristic<State> for TsptwWidth {
    fn max_width(&self, state: &State) -> usize {
        self.nb_vars * (state.depth as usize + 1) * self.factor
    }
}
