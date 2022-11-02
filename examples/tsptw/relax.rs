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

//! This module contains the definition and implementation of the relaxation 
//! for the TSP + TW problem.

use std::{ops::Not};

use bitset_fixed::BitSet;
use engineering::{Relaxation, Decision, Problem};

use crate::{model::Tsptw, state::{ElapsedTime, Position, State}};

#[derive(Clone)]
pub struct TsptwRelax<'a> {
    pb : &'a Tsptw,
}
impl <'a> TsptwRelax<'a> {
    pub fn new(pb: &'a Tsptw) -> Self {
        Self{pb}
    }
}
#[derive(Clone)]
struct RelaxHelper {
    depth    : u16,
    position : BitSet,
    earliest : usize,
    latest   : usize,
    all_must : BitSet,
    all_agree: BitSet,
    all_maybe: BitSet,
}
impl RelaxHelper {
    fn new(n: usize) -> Self {
        Self {
            depth    : 0_u16,
            position : BitSet::new(n),
            earliest : usize::max_value(),
            latest   : usize::min_value(),
            all_must : BitSet::new(n),
            all_agree: BitSet::new(n).not(),
            all_maybe: BitSet::new(n),
        }
    }
    fn track_depth(&mut self, depth: u16) {
        self.depth = self.depth.max(depth);
    }
    fn track_position(&mut self, pos: &Position) {
        match pos {
            Position::Node(x)     => self.position.set(*x as usize, true),
            Position::Virtual(xs) => self.position |= xs,
        };
    }
    fn track_elapsed(&mut self, elapsed: ElapsedTime) {
        match elapsed {
            ElapsedTime::FixedAmount{duration} => {
                self.earliest = self.earliest.min(duration);
                self.latest   = self.latest.max(duration);
            },
            ElapsedTime::FuzzyAmount{earliest: ex, latest: lx} => {
                self.earliest = self.earliest.min(ex);
                self.latest   = self.latest.max(lx);
            }
        };
    }
    fn track_must_visit(&mut self, bs: &BitSet) {
        self.all_agree &= bs;
        self.all_must  |= bs;
    }
    fn track_maybe(&mut self, bs: &Option<BitSet>) {
        if let Some(bs) = bs.as_ref() {
            self.all_maybe |= bs;
        }
    }

    fn get_depth(&self) -> u16 {
        self.depth
    }
    fn get_position(&self) -> Position {
        Position::Virtual(self.position.clone())
    }
    fn get_elapsed(&self) -> ElapsedTime {
        if self.earliest == self.latest {
            ElapsedTime::FixedAmount {duration: self.earliest}
        } else {
            ElapsedTime::FuzzyAmount {earliest: self.earliest, latest: self.latest}
        }
    }
    fn get_must_visit(&self) -> BitSet {
        self.all_agree.clone()
    }
    fn get_maybe_visit(&self)-> Option<BitSet> {
        let mut maybe = self.all_maybe.clone(); // three lines: faster because it is in-place
        maybe |= &self.all_must;
        maybe ^= &self.all_agree;

        let count = maybe.count_ones();
        if count > 0 {
            Some(maybe)
        } else {
            None
        }
    }
}

impl Relaxation for TsptwRelax<'_> {
    type State = State;

    fn merge(&self, states: &mut dyn Iterator<Item = &State>) -> State {
        let mut helper = RelaxHelper::new(self.pb.nb_variables());

        for state in states {
            helper.track_depth(state.depth);
            helper.track_position(&state.position);
            helper.track_elapsed(state.elapsed);
            helper.track_must_visit(&state.must_visit);
            helper.track_maybe(&state.maybe_visit);
        }

        State {
            depth      : helper.get_depth(),
            position   : helper.get_position(),
            elapsed    : helper.get_elapsed(),
            must_visit : helper.get_must_visit(),
            maybe_visit: helper.get_maybe_visit(),
        }
    }

    fn relax(&self, _: &State, _: &State, _: &State, _: Decision, cost: isize) -> isize {
        cost
    }
}
