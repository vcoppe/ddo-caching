//! This module contains general purpose useful stuffs
//! 

use std::{iter::Cloned, cmp::Ordering, slice::Iter, ops::{Index, IndexMut}};

use bitset_fixed::BitSet;
/// This structure defines an iterator capable of iterating over the 1-bits of
/// a fixed bitset. It uses word representation of the items in the set, so it
/// should be more efficient to use than a crude iteration over the elements of
/// the set.
///
/// # Example
/// ```
/// # use bitset_fixed::BitSet;
/// # use engineering::utils::BitSetIter;
///
/// let mut bit_set = BitSet::new(5);
/// bit_set.set(1, true);
/// bit_set.set(2, true);
/// bit_set.set(4, true);
///
/// // Successively prints 1, 2, 4
/// for x in BitSetIter::new(&bit_set) {
///     println!("{}", x);
/// }
/// ```
///
pub struct BitSetIter<'a> {
    /// An iterator over the buffer of words of the bitset
    iter: Cloned<Iter<'a, u64>>,
    /// The current word (or none if we exhausted all iterations)
    word: Option<u64>,
    /// The value of position 0 in the current word
    base: usize,
    /// An offset in the current word
    offset: usize,
}
impl BitSetIter<'_> {
    /// This method creates an iterator for the given bitset from an immutable
    /// reference to that bitset.
    pub fn new(bs: &BitSet) -> BitSetIter {
        let mut iter = bs.buffer().iter().cloned();
        let word = iter.next();
        BitSetIter {iter, word, base: 0, offset: 0}
    }
}
/// `BitSetIter` is an iterator over the one bits of the bitset. As such, it
/// implements the standard `Iterator` trait.
impl Iterator for BitSetIter<'_> {
    type Item = usize;

    /// Returns the nex element from the iteration, or None, if there are no more
    /// elements to iterate upon.
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(w) = self.word {
            if w == 0 || self.offset >= 64 {
                self.word   = self.iter.next();
                self.base  += 64;
                self.offset = 0;
            } else {
                let mut mask = 1_u64 << self.offset as u64;
                while (w & mask) == 0 && self.offset < 64 {
                    mask <<= 1;
                    self.offset += 1;
                }
                if self.offset < 64 {
                    let ret = Some(self.base + self.offset);
                    self.offset += 1;
                    return ret;
                }
            }
        }
        None
    }
}

/// A totally ordered Bitset wrapper. Useful to implement tie break mechanisms.
/// This wrapper orders the bitsets according to the lexical order of their
/// underlying bits.
///
/// # Note:
/// This implementation uses the underlying _words_ representation of the
/// bitsets to perform several comparisons at once. Hence, using a `LexBitSet`
/// should be more efficient than trying to establish the total ordering
/// yourself with a loop on the 1-bits of the two sets.
///
/// # Example
/// ```
/// # use bitset_fixed::BitSet;
/// # use engineering::utils::LexBitSet;
///
/// let mut a = BitSet::new(5);
/// let mut b = BitSet::new(5);
///
/// a.set(2, true);  // bits 0..2 match for a and b
/// b.set(2, true);
///
/// a.set(3, false); // a and b diverge on bit 3
/// b.set(3, true);  // and a has a 0 bit in that pos
///
/// a.set(4, true);  // anything that remains after
/// b.set(4, false); // the firs lexicographical difference is ignored
///
/// assert!(LexBitSet(&a) < LexBitSet(&b));
/// ```
///
#[derive(Debug)]
pub struct LexBitSet<'a>(pub &'a BitSet);

/// The `LexBitSet` implements a total order on bitsets. As such, it must
/// implement the standard trait `Ord`.
///
/// # Note:
/// This implementation uses the underlying _words_ representation of the
/// bitsets to perform several comparisons at once. Hence, using a `LexBitSet`
/// should be more efficient than trying to establish the total ordering
/// yourself with a loop on the 1-bits of the two sets.
impl Ord for LexBitSet<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut x = self.0.buffer().iter().cloned();
        let mut y = other.0.buffer().iter().cloned();
        let end   = x.len().max(y.len());

        for _ in 0..end {
            let xi = x.next().unwrap_or(0);
            let yi = y.next().unwrap_or(0);
            if xi != yi {
                let mut mask = 1_u64;
                for _ in 0..64 {
                    let bit_x = xi & mask;
                    let bit_y = yi & mask;
                    if bit_x != bit_y {
                        return bit_x.cmp(&bit_y);
                    }
                    mask <<= 1;
                }
            }
        }
        Ordering::Equal
    }
}

/// Because it is a total order, `LexBitSet` must also be a partial order.
/// Hence, it must implement the standard trait `PartialOrd`.
impl PartialOrd for LexBitSet<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Because `LexBitSet` defines a total order, it makes sense to consider that
/// it also defines an equivalence relation. As such, it implements the standard
/// `Eq` and `PartialEq` traits.
impl Eq for LexBitSet<'_> {}

/// Having `LexBitSet` to implement `PartialEq` means that it _at least_ defines
/// a partial equivalence relation.
impl PartialEq for LexBitSet<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 || self.cmp(other) == Ordering::Equal
    }
}

/// This structure implements a 2D matrix of size [ n X m ].
///
///
/// # Example
/// ```
/// # use ddo::Matrix;
///
/// let mut adjacency = Matrix::new_default(5, 5, None);
///
/// adjacency[(2, 2)] = Some(-5);
/// assert_eq!(Some(-5), adjacency[(2, 2)]);
/// ```
#[derive(Debug, Clone)]
pub struct Matrix<T> {
    /// The number of rows
    pub n: usize,
    /// The number of columns
    pub m: usize,
    /// The items of the matrix
    pub data : Vec<T>
}
impl <T : Default + Clone> Matrix<T> {
    /// Allows the creation of a matrix initialized with the default element
    pub fn new(m: usize, n: usize) -> Self {
        Matrix { m, n, data: vec![Default::default(); m * n] }
    }
}
impl <T : Clone> Matrix<T> {
    /// Allows the creation of a matrix initialized with the default element
    pub fn new_default(m: usize, n: usize, item: T) -> Self {
        Matrix { m, n, data: vec![item; m * n] }
    }
}
impl <T> Matrix<T> {
    /// Returns the position (offset in the data) of the given index
    fn pos(&self, idx: (usize, usize)) -> usize {
        self.m * idx.0 + idx.1
    }
}
/// A matrix is typically an item you'll want to adress using 2D position
impl <T> Index<(usize, usize)> for Matrix<T> {
    type Output = T;

    /// It returns a reference to some item from the matrix at the given 2D index
    fn index(&self, idx: (usize, usize)) -> &Self::Output {
        let position = self.pos(idx);
        &self.data[position]
    }
}
impl <T> IndexMut<(usize, usize)> for Matrix<T> {
    /// It returns a mutable reference to some item from the matrix at the given 2D index
    fn index_mut(&mut self, idx: (usize, usize)) -> &mut Self::Output {
        let position = self.pos(idx);
        &mut self.data[position]
    }
}