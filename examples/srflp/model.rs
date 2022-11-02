// Copyright 2020 Xavier Gillard
//
// Permission is hereby granted, free of charge, to any person obtaining a copy of
// this software and associated documentation files (the "Software"), to deal in
// the Software without restriction, including without limitation the rights to
// use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software is furnished to do so,
// subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
// FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
// COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
// IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
// CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! This module contains the definition of the dynamic programming formulation 
//! of the SRFLP. (Implementation of the `Problem` trait).

use std::{ops::Not, cmp::Reverse, vec};

use bitset_fixed::BitSet;
use engineering::{BitSetIter, Problem, Decision, Variable};
use ordered_float::OrderedFloat;

use crate::{instance::SrflpInstance, state::State};


/// This is the structure encapsulating the Srflp problem.
#[derive(Debug, Clone)]
pub struct Srflp {
    pub instance: SrflpInstance,
    pub sorted_lengths: Vec<(isize, usize)>,
    pub sorted_flows: Vec<(isize, usize, usize)>,
    pub initial : State,
}
impl Srflp {
    pub fn new(inst: SrflpInstance) -> Self {
        let mut sorted_lengths: Vec<(isize, usize)> = inst.lengths.iter().enumerate().map(|(i,l)| (*l,i)).collect();
        sorted_lengths.sort_unstable();
        let mut sorted_flows = vec![];
        for i in 0..inst.nb_departments {
            for j in (i+1)..inst.nb_departments {
                sorted_flows.push((inst.flows[(i as usize, j as usize)], i as usize, j as usize));
            }
        }
        sorted_flows.sort_unstable();

        let state = State {
            must_place: BitSet::new(inst.nb_departments as usize).not(),
            maybe_place: None,
            cut: vec![0; inst.nb_departments as usize],
            depth : 0
        };
        Self { instance: inst, sorted_lengths: sorted_lengths, sorted_flows: sorted_flows, initial: state }
    }
}

impl Problem for Srflp {
    type State = State;

    fn nb_variables(&self) -> usize {
        self.instance.nb_departments as usize
    }

    fn initial_state(&self) -> State {
        self.initial.clone()
    }

    fn initial_value(&self) -> isize {
        0
    }

    fn for_each_in_domain<F>(&self, var: Variable, state: &Self::State, mut f: F)
    where
        F: FnMut(Decision),
    {
        let mut complete_arrangement = self.nb_variables() - state.depth as usize;

        for i in BitSetIter::new(&state.must_place) {
            complete_arrangement -= 1;
            f(Decision { var, value: i as isize })
        }

        if complete_arrangement > 0 {
            if let Some(maybe_visit) = &state.maybe_place {
                for i in BitSetIter::new(maybe_visit) {
                    f(Decision { var, value: i as isize })
                }
            }
        }
    }

    fn transition(&self, state: &State, d: Decision) -> State {
        let d = d.value as usize;

        // if it is a true move
        let mut remaining = state.must_place.clone();
        remaining.set(d, false);
        // if it is a possible move
        let mut maybes = state.maybe_place.clone();
        if let Some(maybe) = maybes.as_mut() {
            maybe.set(d, false);
        }

        let mut cut = state.cut.clone();
        cut[d] = 0;

        for i in BitSetIter::new(&remaining) {
            cut[i] += self.instance.flows[(d, i)];
        }

        if let Some(maybe) = maybes.as_ref() {
            for i in BitSetIter::new(&maybe) {
                cut[i] += self.instance.flows[(d, i)];
            }
        }

        State {
            must_place: remaining,
            maybe_place: maybes,
            cut: cut,
            depth: state.depth + 1
        }
    }

    fn transition_cost(&self, state: &State, d: Decision) -> isize {
        let d = d.value as usize;

        let mut cut = 0;
        let mut complete_arrangement = (self.instance.nb_departments - (state.depth + 1)) as usize;

        for i in BitSetIter::new(&state.must_place) {
            if i != d {
                cut += state.cut[i];
                complete_arrangement -= 1;
            }
        }

        if complete_arrangement > 0 {
            if let Some(maybe) = state.maybe_place.as_ref() {
                let mut temp = vec![];
                for i in BitSetIter::new(&maybe) {
                    if i != d {
                        temp.push(state.cut[i]);
                    }
                }
                temp.sort_unstable();
                cut += temp.iter().take(complete_arrangement).sum::<isize>();
            }
        }

        // Srflp is a minimization problem but the solver works with a 
        // maximization perspective. So we have to negate the cost.
        - cut * self.instance.lengths[d]
    }

    fn next_variable(&self, next_layer: &mut dyn Iterator<Item = &Self::State>)
        -> Option<Variable> {
        let state = next_layer.next();
        if let Some(s) = state {
            let depth = s.depth as usize;
            if depth == self.nb_variables() {
                None
            } else {
                Some(Variable(depth))
            }
        } else {
            None
        }
    }

    fn estimate(&self, state: &State) -> isize {
        let complete_arrangement = self.nb_variables() - state.depth as usize;
        let n_flows = complete_arrangement * (complete_arrangement - 1) / 2;
        let n_must_place = state.must_place.count_ones() as usize;
        let n_from_maybe_place = complete_arrangement - n_must_place;

        let mut ratios = vec![];
        let mut flows = vec![];
        let mut lengths = vec![];
        let mut maybe_lengths = vec![];

        let mut n_lengths_from_maybe_place = n_from_maybe_place;
        for (l,i) in self.sorted_lengths.iter() {
            if state.must_place[*i] {
                lengths.push(*l);
            } else if let Some(maybe) = state.maybe_place.as_ref() {
                if maybe[*i] && n_lengths_from_maybe_place > 0 {
                    lengths.push(*l);
                    maybe_lengths.push(*l);
                    n_lengths_from_maybe_place -= 1;
                }
            }
            if lengths.len() == complete_arrangement {
                break;
            }
        }

        let mut n_flows_from_must_to_maybe_place = n_must_place * n_from_maybe_place;
        let mut n_flows_in_maybe_place = n_from_maybe_place * (n_from_maybe_place - 1) / 2;
        for (f,i,j) in self.sorted_flows.iter() {
            if state.must_place[*i] && state.must_place[*j] {
                flows.push(*f);
            } else if let Some(maybe) = state.maybe_place.as_ref() {
                if state.must_place[*i] && maybe[*j] && n_flows_from_must_to_maybe_place > 0 {
                    flows.push(*f);
                    n_flows_from_must_to_maybe_place -= 1;
                } else if maybe[*i] && state.must_place[*j] && n_flows_from_must_to_maybe_place > 0 {
                    flows.push(*f);
                    n_flows_from_must_to_maybe_place -= 1;
                } else if maybe[*i] && maybe[*j] && n_flows_in_maybe_place > 0 {
                    flows.push(*f);
                    n_flows_in_maybe_place -= 1;
                }
            }

            if flows.len() == n_flows {
                break;
            }
        }

        for i in BitSetIter::new(&state.must_place) {
            ratios.push((OrderedFloat((state.cut[i] as f32) / (self.instance.lengths[i] as f32)), self.instance.lengths[i], state.cut[i]));
        }
        
        if let Some(maybe) = state.maybe_place.as_ref() {
            let mut maybe_cuts = vec![];

            for i in BitSetIter::new(maybe) {
                maybe_cuts.push(state.cut[i]);
            }

            maybe_cuts.sort_unstable();

            for i in 0..n_from_maybe_place {
                let l = maybe_lengths[i];
                let c = maybe_cuts[n_from_maybe_place - 1 - i];
                ratios.push((OrderedFloat((c as f32) / (l as f32)), l, c));
            }
        }

        ratios.sort_unstable_by_key(|r| Reverse(*r));

        let mut cut_bound = 0;
        let mut cumul_length = 0;
        for (_, l, c) in ratios.iter() {
            cut_bound += cumul_length * c;
            cumul_length += l;
        }

        let mut edge_bound = 0;
        let mut idx = 0;
        cumul_length = 0;
        for i in 0..(complete_arrangement-1) {
            for _ in 0..(complete_arrangement-(i+1)) {
                edge_bound += cumul_length * flows[n_flows - 1 - idx];
                idx += 1;
            }

            cumul_length += lengths[i];
        }

        - (cut_bound + edge_bound)
    }
}

impl Srflp {
    pub fn _root_value(&self) -> f64 {
        let mut value = 0.0;

        for i in 0..self.instance.nb_departments {
            for j in (i+1)..self.instance.nb_departments {
                value += 0.5 * ((self.instance.lengths[i as usize] + self.instance.lengths[j as usize])
                             * self.instance.flows[(i as usize, j as usize)]) as f64;
            }
        }

        value
    }
}