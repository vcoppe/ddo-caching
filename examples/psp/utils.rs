use std::{
    fmt::{Debug, Display},
    ops::{Index, IndexMut},
};

/// This structure implements a 2D matrix of size [ n X m ].
///
///
/// # Example
/// ```
/// # use psp-parsing::Matrix;
///
/// let mut adjacency = Matrix::new_default(5, 5, None);
///
/// adjacency[(2, 2)] = Some(-5);
/// assert_eq!(Some(-5), adjacency[(2, 2)]);
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Matrix<T> {
    /// The number of rows
    pub n: usize,
    /// The number of columns
    pub m: usize,
    /// The items of the matrix
    pub data: Vec<T>,
}
impl<T: Default + Clone> Matrix<T> {
    /// Allows the creation of a matrix initialized with the default element
    pub fn new(n: usize, m: usize) -> Self {
        Matrix {
            n,
            m,
            data: vec![Default::default(); m * n],
        }
    }

    #[allow(dead_code)]
    pub fn row(&self, i: usize) -> impl Iterator<Item = &T> {
        let start = self.pos((i, 0));
        let end = self.pos((i, self.m));
        self.data[start..end].iter()
    }
    #[allow(dead_code)]
    pub fn row_mut(&mut self, i: usize) -> impl Iterator<Item = &mut T> {
        let start = self.pos((i, 0));
        let end = self.pos((i, self.m));
        self.data[start..end].iter_mut()
    }
    pub fn col(&self, i: usize) -> impl Iterator<Item = &T> {
        (0..self.n).map(move |r| &self.data[self.pos((r, i))])
    }
    #[allow(dead_code)]
    pub fn col_mut(&mut self, i: usize) -> impl Iterator<Item = &mut T> {
        let ptr = self.data.as_mut_ptr();
        (0..self.n).map(move |r| unsafe { ptr.add(self.pos((r, i))).as_mut().unwrap() })
    }
}
impl<T: Clone> Matrix<T> {
    /// Allows the creation of a matrix initialized with the default element
    pub fn new_default(m: usize, n: usize, item: T) -> Self {
        Matrix {
            m,
            n,
            data: vec![item; m * n],
        }
    }
}
impl<T> Matrix<T> {
    /// Returns the position (offset in the data) of the given index
    fn pos(&self, idx: (usize, usize)) -> usize {
        debug_assert!(idx.0 < self.n, "position invalide: m");
        debug_assert!(
            idx.1 < self.m,
            "position invalide: {} >= n {}",
            idx.1,
            self.n
        );
        self.m * idx.0 + idx.1
    }
}
/// A matrix is typically an item you'll want to adress using 2D position
impl<T> Index<(usize, usize)> for Matrix<T> {
    type Output = T;

    /// It returns a reference to some item from the matrix at the given 2D index
    fn index(&self, idx: (usize, usize)) -> &Self::Output {
        let position = self.pos(idx);
        &self.data[position]
    }
}
impl<T> IndexMut<(usize, usize)> for Matrix<T> {
    /// It returns a mutable reference to some item from the matrix at the given 2D index
    fn index_mut(&mut self, idx: (usize, usize)) -> &mut Self::Output {
        let position = self.pos(idx);
        &mut self.data[position]
    }
}

/// Tell the compiler how to visually display the matrix when in debug mode
impl<T: Debug> Debug for Matrix<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, v) in self.data.iter().enumerate() {
            if i % self.m == 0 {
                writeln!(f)?;
            }
            write!(f, " {:>5?}", v)?;
        }
        writeln!(f)
    }
}

/// Tell the compiler how to visually display the matrix
impl<T: Display> Display for Matrix<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, v) in self.data.iter().enumerate() {
            if i % self.m == 0 {
                writeln!(f)?;
            }
            write!(f, " {:>5}", v)?;
        }
        writeln!(f)
    }
}
