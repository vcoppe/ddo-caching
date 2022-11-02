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
//! of the TSP+TW. (Implementation of the `Problem` trait).

use std::ops::Not;

use bitset_fixed::BitSet;
use engineering::{BitSetIter, Problem, Decision, Variable};

use crate::{instance::TsptwInstance, state::{ElapsedTime, Position, State}};


/// This is the structure encapsulating the Tsptw problem.
#[derive(Debug, Clone)]
pub struct Tsptw {
    pub instance: TsptwInstance,
    pub initial : State,
    cheapest_edge: Vec<usize>,
}
impl Tsptw {
    pub fn new(inst: TsptwInstance) -> Self {
        let cheapest_edge = Self::compute_cheapest_edges(&inst);
        let mut state = State {
            position  : Position::Node(0),
            elapsed   : ElapsedTime::FixedAmount{duration: 0},
            must_visit: BitSet::new(inst.nb_nodes as usize).not(),
            maybe_visit: None,
            depth : 0
        };
        state.must_visit.set(0, false);
        Self { instance: inst, initial: state, cheapest_edge }
    }

    fn compute_cheapest_edges(inst: &TsptwInstance) -> Vec<usize> {
        let mut cheapest = vec![];
        let n = inst.nb_nodes as usize;
        for i in 0..n {
            let mut min_i = usize::max_value();
            for j in 0..n {
                if i == j {
                    continue;
                }
                min_i = min_i.min(inst.distances[(j, i)]);
            }
            cheapest.push(min_i);
        }
        cheapest
    }

    pub fn _total_openness(&self, state: &State) -> isize {
        let now = state.depth as usize;
        let mut tot = 0;
        for x in BitSetIter::new(&state.must_visit) {
            let tw = self.instance.timewindows[x];
            let op = tw.latest as isize - tw.earliest.max(now) as isize;
            if op < 0 {
                return isize::MIN;
            } else {
                tot += op;
            }
        }
        tot
    }
}

impl Problem for Tsptw {
    type State = State;

    fn nb_variables(&self) -> usize {
        self.instance.nb_nodes as usize
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
        // When we are at the end of the tour, the only possible destination is
        // to go back to the depot. Any state that violates this constraint is
        // de facto infeasible.
        if state.depth as usize == self.nb_variables() - 1 {
            if self.can_move_to(state, 0) {
                f(Decision { var, value: 0 })
            }
            return;
        }

        for i in BitSetIter::new(&state.must_visit) {
            if !self.can_move_to(state, i) {
                return;
            }
        }
        for i in BitSetIter::new(&state.must_visit) {
            f(Decision { var, value: i as isize })
        }

        // Add those that can possibly be visited
        if let Some(maybe_visit) = &state.maybe_visit {
            for i in BitSetIter::new(maybe_visit) {
                if self.can_move_to(state, i) {
                    f(Decision { var, value: i as isize })
                }
            }
        }
    }

    fn transition(&self, state: &State, d: Decision) -> State {
        // if it is a true move
        let mut remaining = state.must_visit.clone();
        remaining.set(d.value as usize, false);
        // if it is a possible move
        let mut maybes = state.maybe_visit.clone();
        if let Some(maybe) = maybes.as_mut() {
            maybe.set(d.value as usize, false);
        }

        let time = self.arrival_time(state, d.value as usize);

        State {
            position : Position::Node(d.value as u16),
            elapsed  : time,
            must_visit: remaining,
            maybe_visit: maybes,
            depth: state.depth + 1
        }
    }

    fn transition_cost(&self, state: &State, d: Decision) -> isize {
        // Tsptw is a minimization problem but the solver works with a 
        // maximization perspective. So we have to negate the min if we want to
        // yield a lower bound.
        let twj = self.instance.timewindows[d.value as usize];
        let travel_time = self.min_distance_to(state, d.value as usize);
        let waiting_time = match state.elapsed {
            ElapsedTime::FixedAmount{duration} => 
                if (duration + travel_time) < twj.earliest {
                    twj.earliest - (duration + travel_time)
                } else {
                    0
                },
            ElapsedTime::FuzzyAmount{earliest, ..} => 
                if (earliest + travel_time) < twj.earliest {
                    twj.earliest - (earliest + travel_time)
                } else {
                    0
                }
        };

        -( (travel_time + waiting_time) as isize)
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
        let mut complete_tour = self.nb_variables() - state.depth as usize;
 
        let mut mandatory     = 0;
        let mut back_to_depot = usize::max_value();
        
        let mut temp = vec![];
 
        for i in BitSetIter::new(&state.must_visit) {
            complete_tour -= 1;
            mandatory += self.cheapest_edge[i];
            back_to_depot = back_to_depot.min(self.instance.distances[(i, 0)]);
 
            let latest   = self.instance.timewindows[i].latest;
            let earliest = state.elapsed.add_duration(self.cheapest_edge[i]).earliest();
            if earliest > latest {
                return isize::min_value();
            }
        }
 
        if let Some(maybes) = state.maybe_visit.as_ref() {
            let mut violations = 0;

            for i in BitSetIter::new(maybes) {
            temp.push(self.cheapest_edge[i]);
            back_to_depot = back_to_depot.min(self.instance.distances[(i, 0)]);
            
            let latest   = self.instance.timewindows[i].latest;
            let earliest = state.elapsed.add_duration(self.cheapest_edge[i]).earliest();
            if earliest > latest {
                violations += 1;
            }
            }

            if temp.len() - violations < complete_tour {
                return isize::min_value();
            }

            temp.sort_unstable();
            mandatory += temp.iter().copied().take(complete_tour).sum::<usize>();
        }
 
        // When there is no other city that MUST be visited, we must consider 
        // the shortest distance between *here* (current position) and the 
        // depot.
        if mandatory == 0 {
            back_to_depot = back_to_depot.min(
                match &state.position {
                 Position::Node(x) => 
                     self.instance.distances[(*x as usize, 0)],
                 Position::Virtual(bs) =>
                     BitSetIter::new(bs).map(|x| self.instance.distances[(x, 0)]).min().unwrap()
            });
        }
 
        // When it is impossible to get back to the depot in time, the current
        // state is infeasible. So we can give it an infinitely negative ub.
        let total_distance  = mandatory + back_to_depot;
        let earliest_arrival= state.elapsed.add_duration(total_distance).earliest();
        let latest_deadline = self.instance.timewindows[0].latest;
        if earliest_arrival > latest_deadline {
            isize::min_value()
        } else {
             -(total_distance as isize)
        }
    }
}

impl Tsptw {
    pub fn can_move_to(&self, state: &State, j: usize) -> bool {
        let twj         = self.instance.timewindows[j];
        let min_arrival = state.elapsed.add_duration(self.min_distance_to(state, j));
        match min_arrival {
            ElapsedTime::FixedAmount{duration}     => duration <= twj.latest,
            ElapsedTime::FuzzyAmount{earliest, ..} => earliest <= twj.latest,
        }
    }
    fn arrival_time(&self, state: &State, j: usize) -> ElapsedTime {
       let min_arrival = state.elapsed.add_duration(self.min_distance_to(state, j));
       let max_arrival = state.elapsed.add_duration(self.max_distance_to(state, j));

       let min_arrival = match min_arrival {
           ElapsedTime::FixedAmount{duration}     => duration,
           ElapsedTime::FuzzyAmount{earliest, ..} => earliest
       };
       let max_arrival = match max_arrival {
           ElapsedTime::FixedAmount{duration}    => duration,
           ElapsedTime::FuzzyAmount{latest, ..}  => latest
       };
       // This would be the arrival time if we never had to wait.
       let arrival_time = 
           if min_arrival.eq(&max_arrival) { 
               ElapsedTime::FixedAmount{duration: min_arrival} 
           } else {
               ElapsedTime::FuzzyAmount{earliest: min_arrival, latest: max_arrival}
           };
       // In order to account for the possible waiting time, we need to adjust
       // the earliest arrival time
       let twj = self.instance.timewindows[j];
       match arrival_time {
          ElapsedTime::FixedAmount{duration} => {
              ElapsedTime::FixedAmount{duration: duration.max(twj.earliest)}
          },
          ElapsedTime::FuzzyAmount{mut earliest, mut latest} => {
            earliest = earliest.max(twj.earliest);
            latest   = latest.min(twj.latest);

            if earliest.eq(&latest) {
                ElapsedTime::FixedAmount{duration: earliest}
            } else {
                ElapsedTime::FuzzyAmount{earliest, latest}
            }
          },
      }
    }
    fn min_distance_to(&self, state: &State, j: usize) -> usize {
        match &state.position {
            Position::Node(i) => self.instance.distances[(*i as usize, j)],
            Position::Virtual(candidates) => 
                BitSetIter::new(candidates)
                    .map(|i| self.instance.distances[(i as usize, j as usize)])
                    .min()
                    .unwrap()
        }
    }
    fn max_distance_to(&self, state: &State, j: usize) -> usize {
        match &state.position {
            Position::Node(i) => self.instance.distances[(*i as usize, j)],
            Position::Virtual(candidates) => 
                BitSetIter::new(candidates)
                    .map(|i| self.instance.distances[(i as usize, j as usize)])
                    .max()
                    .unwrap()
        }
    }
}
