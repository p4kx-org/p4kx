// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gltf::MorphTarget;
use p4kx_model::{Mesh, Model};
use rayon::prelude::*;

use super::follow::{Grid, Vec3};
use super::head_move::first_mesh;

/// The smallest move of a vertex, in metres, that a morph target of what follows the head keeps.
const REST: f32 = 1e-5;
/// A move under this fraction of the largest move of its target is left out of the target.
const RELATIVE_REST: f32 = 0.2;

/// The morph targets of what follows the head, and the target of the head each stands for.
pub(super) struct Followed {
    pub(super) targets: Vec<MorphTarget>,
    pub(super) of: Vec<usize>,
}

impl Followed {
    pub(super) const fn none() -> Self {
        Self {
            targets: Vec::new(),
            of: Vec::new(),
        }
    }
}

/// The head and the mesh that follows it, with the weight each vertex of the mesh gives the head.
struct Spread<'a> {
    head: &'a Mesh,
    mesh: &'a Mesh,
    mesh_grid: Grid<'a>,
    totals: Vec<f32>,
}

struct Seen {
    marks: Vec<u32>,
    round: u32,
}

impl Seen {
    fn new(vertices: usize) -> Self {
        Self {
            marks: vec![0; vertices],
            round: 0,
        }
    }

    fn next_round(&mut self) -> u32 {
        self.round = self.round.wrapping_add(1);
        if self.round == 0 {
            self.marks.fill(0);
            self.round = 1;
        }
        self.round
    }

    fn first_time(&mut self, vertex: u32, round: u32) -> bool {
        self.marks
            .get_mut(vertex as usize)
            .is_some_and(|mark| std::mem::replace(mark, round) != round)
    }
}

impl Spread<'_> {
    /// Returns the target of what follows the head for the target `target` of the head: the move
    /// of the head's vertices spread to the vertices of the mesh as the neutral shape spreads it.
    ///
    /// Only the moved vertices of the head count to the sum of moves; the weight a mesh vertex
    /// divides it by is that of all head vertices around it, which is the same for every target.
    fn target(&self, target: &MorphTarget, seen: &mut Seen) -> Option<MorphTarget> {
        let (positions, deltas): (Vec<Vec3>, Vec<Vec3>) = target
            .vertices
            .iter()
            .zip(&target.deltas)
            .filter_map(|(&vertex, &delta)| {
                Some((*self.head.positions.get(vertex as usize)?, delta))
            })
            .unzip();
        let moved = Grid::new(&positions);
        let round = seen.next_round();
        let mut near: Vec<u32> = Vec::new();
        for &position in &positions {
            for list in self.mesh_grid.near(position) {
                near.extend(
                    list.iter()
                        .filter(|&&vertex| seen.first_time(vertex, round)),
                );
            }
        }
        near.sort_unstable();
        let mut all: Vec<(u32, Vec3, f32)> = Vec::new();
        for index in near {
            let (Some(&position), Some(&total)) = (
                self.mesh.positions.get(index as usize),
                self.totals.get(index as usize),
            ) else {
                continue;
            };
            let (_, sum) = moved.weigh(&deltas, position);
            let by = sum.map(|value| value / total);
            if by.iter().all(|value| value.is_finite()) {
                let largest = by.iter().fold(0.0_f32, |peak, value| peak.max(value.abs()));
                all.push((index, by, largest));
            }
        }
        let peak = all.iter().fold(0.0_f32, |peak, &(_, _, own)| peak.max(own));
        let cutoff = REST.max(peak * RELATIVE_REST);
        let (vertices, shifts): (Vec<u32>, Vec<Vec3>) = all
            .into_iter()
            .filter(|&(_, _, own)| own > cutoff)
            .map(|(index, by, _)| (index, by))
            .unzip();
        (!vertices.is_empty()).then(|| MorphTarget {
            name: target.name.clone(),
            vertices,
            deltas: shifts,
        })
    }
}

/// Returns the morph targets of the first mesh of `model`, which follows the head whose unshaped
/// mesh is `head`, for the expression targets `targets` of the head: each vertex takes the move of
/// the head's vertices around it, as it does for the neutral shape, see
/// [`follow`](super::follow::follow).
///
/// `model`, `head` and `targets` are at the same level of detail. A target that moves none of the
/// vertices is left out; `of` lists the head target each kept target stands for.
pub(super) fn follow_morphs(model: &Model, head: &Mesh, targets: &[MorphTarget]) -> Followed {
    let Some(mesh) = first_mesh(model) else {
        return Followed::none();
    };
    if targets.is_empty() {
        return Followed::none();
    }
    let head_grid = Grid::new(&head.positions);
    let totals = mesh
        .positions
        .par_iter()
        .map(|&position| head_grid.weigh(&[], position).0)
        .collect();
    let spread = Spread {
        head,
        mesh,
        mesh_grid: Grid::new(&mesh.positions),
        totals,
    };
    let made: Vec<(usize, MorphTarget)> = targets
        .par_iter()
        .enumerate()
        .map_init(
            || Seen::new(mesh.positions.len()),
            |seen, (number, target)| spread.target(target, seen).map(|made| (number, made)),
        )
        .flatten()
        .collect();
    let (of, targets) = made.into_iter().unzip();
    Followed { targets, of }
}

#[cfg(test)]
#[path = "follow_morphs_tests.rs"]
mod tests;
