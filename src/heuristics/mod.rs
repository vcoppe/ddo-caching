use crate::WidthHeuristic;

#[derive(Debug, Clone, Copy)]
pub struct Fixed(pub usize);
impl<T> WidthHeuristic<T> for Fixed {
    fn max_width(&self, _state: &T) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NbUnassigned {
    pub nb_vars: usize,
}
// Implement WidthHeuristic in the various example models
