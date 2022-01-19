// Ewens Pitman attraction partition distribution

use crate::clust::Clustering;
use crate::perm::Permutation;

use rand::prelude::*;
use std::slice;

type SimilarityBorrower<'a> = SquareMatrixBorrower<'a>;

#[derive(Debug, Clone)]
pub struct EpaParameters<'a> {
    similarity: SimilarityBorrower<'a>,
    permutation: Permutation,
    mass: f64,
    discount: f64,
}

impl<'a> EpaParameters<'a> {
    pub fn new(
        similarity: SimilarityBorrower<'a>,
        permutation: Permutation,
        mass: f64,
        discount: f64,
    ) -> Option<Self> {
        if similarity.n_items() != permutation.n_items() {
            None
        } else {
            Some(Self {
                similarity,
                permutation,
                mass,
                discount,
            })
        }
    }

    pub fn shuffle_permutation<T: Rng>(&mut self, rng: &mut T) {
        match std::env::var("DBD_PERMUTATION").as_deref() {
            Ok("shuffle") => self.permutation.shuffle(rng),
            Ok("nearest") => {
                self.permutation = {
                    let mut permutation = Vec::with_capacity(self.permutation.n_items());
                    let mut available: Vec<_> = (0..self.permutation.n_items()).collect();
                    let start = rng.gen_range(0..available.len());
                    let mut current_index = available.swap_remove(start);
                    permutation.push(current_index);
                    while !available.is_empty() {
                        let mut best_value = f64::NEG_INFINITY;
                        let mut best_index = usize::MAX;
                        for (k, i) in available.iter().enumerate() {
                            let candidate = self.similarity[(current_index, *i)];
                            if candidate > best_value {
                                best_value = candidate;
                                best_index = k;
                            }
                        }
                        current_index = available.swap_remove(best_index);
                        permutation.push(current_index);
                    }
                    println!("nearest... {permutation:?}");
                    Permutation::from_vector(permutation).unwrap()
                }
            }
            Ok("randomnearest") => {
                self.permutation = {
                    let mut permutation = Vec::with_capacity(self.permutation.n_items());
                    let mut available: Vec<_> = (0..self.permutation.n_items()).collect();
                    let start = rng.gen_range(0..available.len());
                    let mut current_index = available.swap_remove(start);
                    permutation.push(current_index);
                    while !available.is_empty() {
                        let index_and_weights = available
                            .iter()
                            .map(|i| self.similarity[(current_index, *i)])
                            .enumerate();
                        let (index, _) =
                            Clustering::select(index_and_weights, false, 0, Some(rng), false);
                        current_index = available.swap_remove(index);
                        permutation.push(current_index);
                    }
                    println!("randomnearest... {permutation:?}");
                    Permutation::from_vector(permutation).unwrap()
                }
            }
            _ => self.permutation.shuffle(rng),
        }
    }
}

/// A data structure representing a square matrix.
///
#[derive(Debug)]
pub struct SquareMatrix {
    data: Vec<f64>,
    n_items: usize,
}

impl SquareMatrix {
    pub fn zeros(n_items: usize) -> Self {
        Self {
            data: vec![0.0; n_items * n_items],
            n_items,
        }
    }

    pub fn ones(n_items: usize) -> Self {
        Self {
            data: vec![1.0; n_items * n_items],
            n_items,
        }
    }

    pub fn identity(n_items: usize) -> Self {
        let ni1 = n_items + 1;
        let n2 = n_items * n_items;
        let mut data = vec![0.0; n2];
        let mut i = 0;
        while i < n2 {
            data[i] = 1.0;
            i += ni1
        }
        Self { data, n_items }
    }

    pub fn data(&self) -> &[f64] {
        &self.data[..]
    }

    pub fn data_mut(&mut self) -> &mut [f64] {
        &mut self.data[..]
    }

    pub fn view(&mut self) -> SquareMatrixBorrower {
        SquareMatrixBorrower::from_slice(&self.data[..], self.n_items)
    }

    pub fn n_items(&self) -> usize {
        self.n_items
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SquareMatrixBorrower<'a> {
    data: &'a [f64],
    n_items: usize,
}

impl std::ops::Index<(usize, usize)> for SquareMatrixBorrower<'_> {
    type Output = f64;
    fn index(&self, (i, j): (usize, usize)) -> &Self::Output {
        &self.data[self.n_items * j + i]
    }
}

impl<'a> SquareMatrixBorrower<'a> {
    pub fn from_slice(data: &'a [f64], n_items: usize) -> Self {
        assert_eq!(data.len(), n_items * n_items);
        Self { data, n_items }
    }

    /// # Safety
    ///
    /// You're on your own.
    pub unsafe fn from_ptr(data: *const f64, n_items: usize) -> Self {
        let data = slice::from_raw_parts(data, n_items * n_items);
        Self { data, n_items }
    }

    pub fn n_items(&self) -> usize {
        self.n_items
    }

    /// # Safety
    ///
    /// You're on your own.
    pub unsafe fn get_unchecked(&self, (i, j): (usize, usize)) -> &f64 {
        self.data.get_unchecked(self.n_items * j + i)
    }

    pub fn data(&self) -> &[f64] {
        self.data
    }

    pub fn sum_of_triangle(&self) -> f64 {
        let mut sum = 0.0;
        for i in 0..self.n_items {
            for j in 0..i {
                sum += unsafe { *self.get_unchecked((i, j)) };
            }
        }
        sum
    }

    pub fn sum_of_row_subset(&self, row: usize, columns: &[usize]) -> f64 {
        let mut sum = 0.0;
        for j in columns {
            sum += unsafe { *self.get_unchecked((row, *j)) };
        }
        sum
    }
}

pub fn sample<T: Rng>(parameters: &EpaParameters, rng: &mut T) -> Clustering {
    let ni = parameters.similarity.n_items();
    let mass = parameters.mass;
    let discount = parameters.discount;
    let d2 = (1..ni).fold(
        parameters.similarity[(
            parameters.permutation.get(ni - 1),
            parameters.permutation.get(0),
        )],
        |x, i| {
            x + parameters.similarity[(
                parameters.permutation.get(i - 1),
                parameters.permutation.get(i),
            )]
        },
    ) / (ni as f64);
    println!("----\nd2: {d2}");
    let mut clustering = Clustering::unallocated(ni);
    for i in 0..ni {
        let ii = parameters.permutation.get(i);
        let numerator = if i == 0 {
            d2
        } else {
            parameters.similarity[(ii, parameters.permutation.get(i - 1))]
        };
        let jump_density = d2 / numerator;
        println!("{i} {ii} {jump_density}");
        let qt = clustering.n_clusters() as f64;
        let kt = ((i as f64) - discount * qt)
            / parameters
                .similarity
                .sum_of_row_subset(ii, parameters.permutation.slice_until(i));
        let labels_and_weights = clustering
            .available_labels_for_allocation_with_target(None, ii)
            .map(|label| {
                let n_items_in_cluster = clustering.size_of(label);
                let weight = if n_items_in_cluster == 0 {
                    (mass + discount * qt) * jump_density
                } else {
                    kt * parameters
                        .similarity
                        .sum_of_row_subset(ii, &clustering.items_of(label)[..])
                };
                (label, weight)
            });
        let subset_index = Clustering::select(labels_and_weights, false, 0, Some(rng), false).0;
        clustering.allocate(ii, subset_index);
    }
    clustering
}
