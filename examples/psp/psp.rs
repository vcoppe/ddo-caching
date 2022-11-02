use std::{
    cell::RefCell,
    fmt::Debug,
    fs::File,
    io::{BufRead, BufReader, Lines, Read},
};

use engineering::{
    Decision, NbUnassigned, Problem, Relaxation, StateRanking, Variable, WidthHeuristic,
};

use smallbitset::Set32;
use thread_local::ThreadLocal;

use crate::utils::Matrix;

static IDLE: isize = -1;
static BOT: i32 = -1;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct State {
    time: usize,
    k: i32,
    // for each item i, req[i] denotes the time when the current order must be
    // delivered.
    u: Vec<i32>,
}

#[derive(Debug, Copy, Clone)]
pub struct PspRelax;
impl Relaxation for PspRelax {
    type State = State;

    fn merge(&self, states: &mut dyn Iterator<Item = &Self::State>) -> Self::State {
        let mut time = 0;
        let mut xxx = vec![];
        for state in states {
            time = time.max(state.time);
            if xxx.is_empty() {
                xxx = state.u.clone();
            } else {
                for (u, u_) in xxx.iter_mut().zip(state.u.iter()) {
                    *u = *u_.min(u);
                }
            }
        }
        State {
            time,
            k: BOT,
            u: xxx,
        }
    }

    fn relax(
        &self,
        _: &Self::State,
        _: &Self::State,
        _: &Self::State,
        _: engineering::Decision,
        cost: isize,
    ) -> isize {
        cost
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PspRanking;
impl StateRanking for PspRanking {
    type State = State;

    fn compare(&self, a: &Self::State, b: &Self::State) -> std::cmp::Ordering {
        a.time
            .cmp(&b.time)
            .then_with(|| a.k.cmp(&b.k))
            .then_with(|| a.u.cmp(&b.u))
    }
}
impl WidthHeuristic<State> for NbUnassigned {
    fn max_width(&self, state: &State) -> usize {
        state.time
    }
}
pub struct PspWidth {
    nb_vars: usize,
    factor: usize,
}
impl PspWidth {
    pub fn new(nb_vars: usize, factor: usize) -> PspWidth {
        PspWidth { nb_vars, factor }
    }
}
impl WidthHeuristic<State> for PspWidth {
    fn max_width(&self, _state: &State) -> usize {
        self.nb_vars * self.factor
    }
}

#[derive(Debug)]
pub struct Psp {
    pub optimum: Option<usize>,
    pub nb_periods: usize,
    pub nb_items: usize,
    pub nb_orders: usize,
    pub changeover_cost: Matrix<usize>,
    pub stocking_cost: Vec<usize>,
    // le précédent/suivant est -1 lorsqu'il n'ya plus de deadline
    pub prev_demand: Matrix<i32>,
    pub rem_demand: Matrix<isize>,

    pub mst: Vec<usize>,

    buffer_state: ThreadLocal<RefCell<Vec<i32>>>,
    buffer_time: ThreadLocal<RefCell<Vec<usize>>>,
}

impl Problem for Psp {
    type State = State;

    fn nb_variables(&self) -> usize {
        self.nb_periods
    }

    fn initial_state(&self) -> State {
        let u = Vec::from_iter(self.prev_demand.col(self.nb_periods).copied());
        State {
            time: self.nb_periods,
            k: BOT,
            u,
        }
    }

    fn initial_value(&self) -> isize {
        0
    }

    fn next_variable(&self, states: &mut dyn Iterator<Item = &Self::State>) -> Option<Variable> {
        if let Some(state) = states.next() {
            if state.time > 0 {
                Some(Variable(state.time - 1))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn for_each_in_domain<F>(&self, var: Variable, state: &Self::State, mut f: F)
    where
        F: FnMut(Decision),
    {
        let time = var.0;
        let dom = (0..self.nb_items as isize).filter(move |i| state.u[*i as usize] >= time as i32);
        let rem_demand = (0..self.nb_items as usize).map(|i| {
            if state.u[i] < 0 {
                0
            } else {
                self.rem_demand[(i, state.u[i] as usize)] as usize
            }
        }).sum::<usize>();
        
        if rem_demand > time + 1 {
            return;
        }

        for val in dom {
            f(Decision { var, value: val })
        }

        if rem_demand < time + 1 {
            f(Decision { var, value: IDLE })
        }
    }
    fn transition(&self, state: &Self::State, decision: Decision) -> Self::State {
        let mut next = state.clone();
        next.time -= 1;
        if decision.value != IDLE {
            let item = decision.value as usize;
            next.k = item as i32;
            next.u[item] = self.prev_demand[(item, state.u[item] as usize)];
        }
        next
    }

    fn transition_cost(&self, state: &Self::State, decision: Decision) -> isize {
        if decision.value == IDLE {
            0
        } else {
            let time = decision.var.0;
            let item = decision.value as usize;
            let changeover = if state.k == BOT {
                0
            } else {
                self.changeover_cost[(item, state.k as usize)]
            };
            let stocking = self.stocking_cost[item] * (state.u[item] as usize - time);
            -((changeover + stocking) as isize)
        }
    }

    fn estimate(&self, state: &Self::State) -> isize {
        if state.time == 0 {
            0
        } else {
            // This is ugly as a sin: but it works like hell !
            // I simply copy the current state in a buffer which I then pass on to
            // the greedy estimate computation function. Also, I pass on a mutable
            // pointer to the 'mut_time' which is used during the computation of
            // the optimal stocking plan
            let mut mut_time = self
                .buffer_time
                .get_or(|| RefCell::new(vec![0; self.nb_periods]))
                .borrow_mut();
            let mut mut_state = self
                .buffer_state
                .get_or(|| RefCell::new(vec![0; self.nb_items]))
                .borrow_mut();

            mut_state
                .iter_mut()
                .zip(state.u.iter())
                .for_each(|(d, s)| *d = *s);
            let greedy = Self::compute_ideal_stocking(
                state.time,
                mut_state.as_mut(),
                mut_time.as_mut(),
                &self.prev_demand,
                &self.stocking_cost,
            );

            let idx: u32 = Self::vertices(state.k, &state.u).into();
            let mst = self.mst[idx as usize];
            let stock = greedy;

            (stock + mst) as isize
        }
    }
}

impl Psp {
    /*** ESTIMATION ON THE STOCKING COSTS ***************************************/
    fn compute_ideal_stocking(
        periods: usize,
        state: &mut [i32],
        buffer_time: &mut [usize],
        prev_dem: &Matrix<i32>,
        stocking: &[usize],
    ) -> usize {
        for (time, storage_cost) in buffer_time.iter_mut().enumerate().take(periods).rev() {
            let mut item = IDLE;
            let mut deadline = 0_usize;
            let mut cost = 0;

            for (state_item, state_deadline) in state.iter().enumerate() {
                if *state_deadline >= time as i32 && stocking[state_item] >= cost {
                    item = state_item as isize;
                    deadline = *state_deadline as usize;
                    cost = stocking[state_item];
                }
            }

            *storage_cost = (deadline - time) * cost;
            if item != IDLE {
                let item = item as usize;
                state[item] = prev_dem[(item, deadline)];
            }
        }

        // Cumulative sum
        let mut tot: usize = 0;
        for v in buffer_time.iter_mut() {
            tot = tot.saturating_add(*v);
            *v = tot;
        }

        buffer_time[periods - 1]
    }

    /*** ESTIMATION ON THE CHANGEOVER COSTS *************************************/
    fn vertices(prev: i32, requests: &[i32]) -> Set32 {
        let mut vertices = Set32::empty();
        if prev != -1 {
            vertices = vertices.insert(prev as u8);
        }
        for (i, v) in requests.iter().copied().enumerate() {
            if v >= 0 {
                vertices = vertices.insert(i as u8);
            }
        }
        vertices
    }

    fn precompute_all_mst(n_vars: usize, changeover: &Matrix<usize>) -> Vec<usize> {
        let len = 2_usize.pow(n_vars as u32);
        let mut out = vec![0; len];

        let mut heap = vec![];
        for (i, v) in out.iter_mut().enumerate() {
            *v = Self::mst(Set32::from(i as u32), changeover, &mut heap);
        }

        out
    }
    fn mst(
        mut vertices: Set32,
        changeover: &Matrix<usize>,
        heap: &mut Vec<(usize, u8, u8)>,
    ) -> usize {
        for i in vertices {
            for j in vertices {
                if i != j {
                    let a = i as usize;
                    let b = j as usize;
                    let edge = changeover[(a, b)].min(changeover[(b, a)]);
                    heap.push((edge, i, j));
                }
            }
        }
        heap.sort_unstable_by_key(|x| x.0);
        let mut total = 0;
        let mut edge_max = 0;
        let mut iter_heap = heap.iter();
        while !vertices.is_empty() {
            if let Some(edge) = iter_heap.next() {
                let l = edge.0;
                let i = edge.1;
                let j = edge.2;

                if vertices.contains(i) || vertices.contains(j) {
                    edge_max = edge_max.max(l);
                    total += l;
                    vertices = vertices.remove(i);
                    vertices = vertices.remove(j);
                }
            } else {
                break;
            }
        }

        total - edge_max
    }
}

/*** BELOW THIS LINE IS THE CODE TO PARSE INSTANCE FILES *********************/
#[derive(Debug, thiserror::Error)]
pub enum PspError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("missing {0}")]
    Missing(&'static str),
    #[error("expected int {0}")]
    ParseInt(#[from] std::num::ParseIntError),
}
impl TryFrom<File> for Psp {
    type Error = PspError;

    fn try_from(file: File) -> Result<Psp, PspError> {
        Psp::try_from(BufReader::new(file))
    }
}
impl<S: Read> TryFrom<BufReader<S>> for Psp {
    type Error = PspError;

    fn try_from(buf: BufReader<S>) -> Result<Psp, PspError> {
        Psp::try_from(buf.lines())
    }
}
impl<B: BufRead> TryFrom<Lines<B>> for Psp {
    type Error = PspError;

    fn try_from(mut lines: Lines<B>) -> Result<Psp, PspError> {
        let nb_periods = lines
            .next()
            .ok_or(PspError::Missing("nb periods"))??
            .parse::<usize>()?;
        let nb_items = lines
            .next()
            .ok_or(PspError::Missing("nb items"))??
            .parse::<usize>()?;
        let nb_orders = lines
            .next()
            .ok_or(PspError::Missing("nb orders"))??
            .parse::<usize>()?;

        let _blank = lines.next();
        let mut changeover_cost = Matrix::new_default(nb_items, nb_items, 0);

        let mut i = 0;
        for line in &mut lines {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                break;
            }

            let costs = line.split_whitespace();
            for (other, cost) in costs.enumerate() {
                changeover_cost[(i, other)] = cost.parse::<usize>()?;
            }

            i += 1;
        }

        let stocking_texts = lines.next().ok_or(PspError::Missing("stocking costs"))??;
        let mut stocking_cost = vec![0; nb_items];
        let stock_iter = stocking_cost
            .iter_mut()
            .zip(stocking_texts.split_whitespace());

        for (cost, text) in stock_iter {
            *cost = text.parse::<usize>()?;
        }

        let _blank = lines.next();

        let mut prev_demand = Matrix::new(nb_items, nb_periods + 1);
        let mut rem_demand: Matrix<isize> = Matrix::new(nb_items, nb_periods);
        i = 0;
        for line in &mut lines {
            let line = line?;
            let line = line.trim();

            if line.is_empty() {
                break;
            }

            let demands_for_item = line.split_whitespace();

            // on construit la relation prev_demand[i]
            let mut last_period = BOT;
            for (period, demand_text) in demands_for_item.enumerate() {
                prev_demand[(i, period)] = last_period;

                if period > 0 {
                    rem_demand[(i, period)] = rem_demand[(i, period - 1)];
                }

                let demand = demand_text.parse::<usize>()?;
                if demand > 0 {
                    last_period = period as i32;
                    rem_demand[(i, period)] += 1;
                }

                if period == nb_periods - 1 {
                    prev_demand[(i, 1 + period)] = last_period;
                }
            }

            i += 1;
        }

        // This means there mus be TWO blank lines between the end of demands
        // and the known optimum.
        let _skip = lines.next();
        let optimum = if let Some(line) = lines.next() {
            Some(line?.trim().parse::<usize>()?)
        } else {
            None
        };

        let mst = Psp::precompute_all_mst(nb_items, &changeover_cost);

        Ok(Psp {
            optimum,
            nb_periods,
            nb_items,
            nb_orders,
            changeover_cost,
            stocking_cost,
            prev_demand,
            rem_demand,

            mst,

            buffer_state: ThreadLocal::new(), //RefCell::new(vec![0; nb_items]),
            buffer_time: ThreadLocal::new(),  //RefCell::new(vec![0; nb_periods]),
        })
    }
}
