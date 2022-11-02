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
//! for the SRFLP problem.

use std::{ops::Not};

use bitset_fixed::BitSet;
use engineering::{Relaxation, Decision, BitSetIter};

use crate::{model::Srflp, state::State};

#[derive(Clone)]
pub struct SrflpRelax<'a> {
    pb : &'a Srflp,
}
impl <'a> SrflpRelax<'a> {
    pub fn new(pb: &'a Srflp) -> Self {
        Self{pb}
    }
}
#[derive(Clone)]
struct RelaxHelper {
    depth    : usize,
    all_must : BitSet,
    all_agree: BitSet,
    all_maybe: BitSet,
    cut      : Vec<isize>,
}
impl RelaxHelper {
    fn new(n: usize) -> Self {
        Self {
            depth    : 0,
            all_must : BitSet::new(n),
            all_agree: BitSet::new(n).not(),
            all_maybe: BitSet::new(n),
            cut      : vec![isize::MAX; n],
        }
    }
    fn track_depth(&mut self, depth: usize) {
        self.depth = self.depth.max(depth);
    }
    fn track_must_visit(&mut self, bs: &BitSet) {
        self.all_agree &= bs;
        self.all_must  |= bs;
    }
    fn track_maybe_visit(&mut self, bs: &Option<BitSet>) {
        if let Some(bs) = bs.as_ref() {
            self.all_maybe |= bs;
        }
    }
    fn track_cut(&mut self, state: &State) {
        for i in BitSetIter::new(&state.must_place) {
            self.cut[i] = self.cut[i].min(state.cut[i]);
        }

        if let Some(maybe) = state.maybe_place.as_ref() {
            for i in BitSetIter::new(maybe) {
                self.cut[i] = self.cut[i].min(state.cut[i]);
            }
        }
    }

    fn get_depth(&self) -> usize {
        self.depth
    }
    fn get_must_place(&self) -> BitSet {
        self.all_agree.clone()
    }
    fn get_maybe_place(&self)-> Option<BitSet> {
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
    fn get_cut(&self)-> Vec<isize> {
        self.cut.clone()
    }
}

impl Relaxation for SrflpRelax<'_> {
    type State = State;

    fn merge(&self, states: &mut dyn Iterator<Item = &State>) -> State {
        let mut helper = RelaxHelper::new(self.pb.instance.nb_departments as usize);

        for state in states {
            helper.track_depth(state.depth);
            helper.track_must_visit(&state.must_place);
            helper.track_maybe_visit(&state.maybe_place);
            helper.track_cut(&state);
        }

        State {
            depth      : helper.get_depth(),
            must_place : helper.get_must_place(),
            maybe_place: helper.get_maybe_place(),
            cut        : helper.get_cut(),
        }
    }

    fn relax(
        &self,
        _: &Self::State,
        _: &Self::State,
        _: &Self::State,
        _: Decision,
        cost: isize,
    ) -> isize
    {
        cost
    }
}
