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

//! This module contains everything that is necessary to parse a SRFLP instance
//! and turn it into a structs usable in Rust. Chances are high that this 
//! module will be of little to no interest to you.

use std::{fs::File, io::{BufRead, BufReader, Lines, Read}};

use engineering::Matrix;

/// This structure represents the SRFLP instance.
#[derive(Debug, Clone)]
pub struct SrflpInstance {
    /// The number of departments
    pub nb_departments: usize, 
    /// This length of each departments
    pub lengths       : Vec<isize>,
    /// This is the flow matrix between any two departments
    pub flows         : Matrix<isize>,
}

impl From<File> for SrflpInstance {
    fn from(file: File) -> Self {
        Self::from(BufReader::new(file))
    }
}
impl <S: Read> From<BufReader<S>> for SrflpInstance {
    fn from(buf: BufReader<S>) -> Self {
        Self::from(buf.lines())
    }
}
impl <B: BufRead> From<Lines<B>> for SrflpInstance {
    fn from(lines: Lines<B>) -> Self {
        let mut lc = 0;
        let mut nb_departments = 0;
        let mut lengths = vec![];
        let mut flows = Matrix::new_default(nb_departments as usize, nb_departments as usize, 0);

        for line in lines {
            let line = line.unwrap();
            let line = line.trim();

            // skip empty lines
            if line.is_empty() {
                continue;
            }
            
           // First line is the number of nodes
            if lc == 0 { 
                nb_departments  = line.split(&[' ',',','\t']).filter(|s| !s.is_empty()).next().unwrap().to_string().parse::<usize>().unwrap();
                flows = Matrix::new_default(nb_departments as usize, nb_departments as usize, 0);
            } 
            // Second line contains the lengths
            else if lc == 1 {
                line.split(&[' ',',','\t']).filter(|s| !s.is_empty()).for_each(|l| {
                    let length = l.to_string().parse::<isize>().unwrap();
                    lengths.push(length);
                });
            }
            // The next 'nb_nodes' lines represent the distances matrix
            else if (2..=(nb_departments+1)).contains(&lc) {
                let i = (lc - 2) as usize;
                for (j, flow) in line.split(&[' ',',','\t']).filter(|s| !s.is_empty()).enumerate() {
                    let flow = flow.to_string().parse::<isize>().unwrap();
                    flows[(i, j)] = flow;
                }
            }
            
            lc += 1;
        }

        // handle asymmetrical flows
        for i in 0..(nb_departments as usize) {
            for j in (i+1)..(nb_departments as usize) {
                if flows[(i, j)] != flows[(j, i)] {
                    flows[(i, j)] += flows[(j, i)];
                    flows[(j, i)] = flows[(i, j)];
                }
            }
        }

        SrflpInstance{nb_departments, lengths, flows}
    }
}
