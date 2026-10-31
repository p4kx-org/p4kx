// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_model::{Mesh, Model};
use rayon::prelude::*;

use super::follow_joints::{HeadSkin, bind};

pub(super) type Vec3 = [f32; 3];

/// The distance beyond which a head vertex no longer moves a vertex of hair, in metres.
const RANGE: f32 = 0.1;
/// The distance at which the head's moves count half, in metres.
const HALF: f32 = 0.05;
/// Keeps the weight of a vertex that lies on the head finite, in metres.
const SOFTEN: f32 = 0.004;
/// The weight of the anchor that does not move.
pub(super) const ANCHOR: f32 = 1.0 / (HALF * HALF);

type Cell = [i32; 3];

fn cell_of(position: Vec3) -> Option<Cell> {
    let at = |value: f32| {
        let cell = (value / RANGE).floor();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a coordinate that large is not a position of a head"
        )]
        (cell.is_finite() && cell.abs() < 1e6).then_some(cell as i32)
    };
    Some([at(position[0])?, at(position[1])?, at(position[2])?])
}

/// Returns the cells around `cell` and `cell` itself.
fn around([cx, cy, cz]: Cell) -> impl Iterator<Item = Cell> {
    (cx - 1..=cx + 1).flat_map(move |x| {
        (cy - 1..=cy + 1).flat_map(move |y| (cz - 1..=cz + 1).map(move |z| [x, y, z]))
    })
}

/// The vertices of a mesh in a grid of cells of [`RANGE`].
pub(super) struct Grid<'a> {
    positions: &'a [Vec3],
    cells: HashMap<Cell, Vec<u32>>,
}

impl<'a> Grid<'a> {
    pub(super) fn new(positions: &'a [Vec3]) -> Self {
        let mut cells: HashMap<Cell, Vec<u32>> = HashMap::new();
        for (index, &position) in positions.iter().enumerate() {
            if let (Some(cell), Ok(index)) = (cell_of(position), u32::try_from(index)) {
                cells.entry(cell).or_default().push(index);
            }
        }
        Self { positions, cells }
    }

    /// Returns the move of `position` given the move `moves` of each vertex of the grid: the
    /// average of the moves of the vertices within [`RANGE`], weighted by the inverse square of
    /// their distance, against an anchor that does not move; a position far from the mesh stays
    /// where it is.
    pub(super) fn at(&self, moves: &[Vec3], position: Vec3) -> Vec3 {
        let (weights, sum) = self.weigh(moves, position);
        sum.map(|value| value / weights)
    }

    /// Returns the weight of the anchor and of all vertices within [`RANGE`] of `position`, and
    /// the sum of the moves `moves` of those vertices times their weights; a vertex without a move
    /// counts to the weight only.
    pub(super) fn weigh(&self, moves: &[Vec3], position: Vec3) -> (f32, Vec3) {
        let mut weights = ANCHOR;
        let mut sum = [0.0_f32; 3];
        self.visit(position, |at, weight| {
            weights += weight;
            if let Some(by) = moves.get(at) {
                for (total, value) in sum.iter_mut().zip(by) {
                    *total += weight * value;
                }
            }
        });
        (weights, sum)
    }

    /// Calls `each` with the index and the weight of every vertex within [`RANGE`] of `position`,
    /// in a fixed order; the weight is that of [`Self::weigh`], without the anchor's.
    pub(super) fn visit(&self, position: Vec3, mut each: impl FnMut(usize, f32)) {
        let Some(cell) = cell_of(position) else {
            return;
        };
        for near in around(cell) {
            let Some(list) = self.cells.get(&near) else {
                continue;
            };
            for &index in list {
                let at = index as usize;
                let Some(head) = self.positions.get(at) else {
                    continue;
                };
                let squared: f32 = head
                    .iter()
                    .zip(&position)
                    .map(|(at, from)| (at - from).powi(2))
                    .sum();
                if squared > RANGE * RANGE {
                    continue;
                }
                let fade = 1.0 - squared / (RANGE * RANGE);
                each(at, fade * fade / SOFTEN.mul_add(SOFTEN, squared));
            }
        }
    }

    /// Returns the cells of the grid within one cell of `position`.
    pub(super) fn near(&self, position: Vec3) -> impl Iterator<Item = &[u32]> {
        cell_of(position)
            .into_iter()
            .flat_map(around)
            .filter_map(|cell| self.cells.get(&cell).map(Vec::as_slice))
    }
}

/// The vertices of the unshaped head in a [`Grid`], and the move of each.
struct Field<'a> {
    grid: Grid<'a>,
    moves: Vec<Vec3>,
}

impl<'a> Field<'a> {
    fn new(before: &'a [Vec3], after: &[Vec3]) -> Self {
        let moves = before
            .iter()
            .zip(after)
            .map(|(was, now)| [now[0] - was[0], now[1] - was[1], now[2] - was[2]])
            .collect();
        Self {
            grid: Grid::new(before),
            moves,
        }
    }

    fn at(&self, position: Vec3) -> Vec3 {
        self.grid.at(&self.moves, position)
    }
}

fn follow_mesh(mesh: &mut Mesh, field: &Field<'_>) {
    mesh.positions.par_iter_mut().for_each(|position| {
        let by = field.at(*position);
        for (value, delta) in position.iter_mut().zip(by) {
            *value += delta;
        }
    });
}

/// Returns `model` with its vertices moved as the head moved from `before` to `after`: the hair,
/// brows, lashes and beard of a Character follow the shape its DNA gives the head.
///
/// A vertex takes the average move of the head vertices within a tenth of a metre, nearer ones
/// counting more, and fades out against a fixed anchor, so hair that hangs far from the head
/// stays where it was. `before` and `after` must be the same mesh; with different vertex counts
/// the model is returned as it is.
///
/// With the skin `skin` of the head, the vertices are also bound to the head's joints, see
/// [`bind`].
pub(super) fn follow(
    model: &Model,
    before: &Mesh,
    after: &Mesh,
    skin: Option<&HeadSkin<'_>>,
) -> Model {
    let mut moved = model.clone();
    if before.positions.len() != after.positions.len() {
        return moved;
    }
    let field = Field::new(&before.positions, &after.positions);
    if let Some(skin) = skin {
        bind(&mut moved, &field.grid, skin);
    }
    if let Some(lod) = moved.lods.first_mut() {
        for mesh in &mut lod.meshes {
            follow_mesh(mesh, &field);
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::Field;

    fn head() -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
        let before = vec![[0.0, 0.0, 0.0], [0.01, 0.0, 0.0]];
        let after = vec![[0.0, 0.0, 0.02], [0.01, 0.0, 0.02]];
        (before, after)
    }

    #[test]
    fn a_vertex_on_the_head_moves_with_it() {
        let (before, after) = head();
        let field = Field::new(&before, &after);

        let [x, y, z] = field.at([0.0, 0.0, 0.0]);

        assert!(x.abs() < 1e-4 && y.abs() < 1e-4, "{x} {y}");
        assert!(z > 0.0195, "{z}");
    }

    #[test]
    fn a_vertex_far_from_the_head_stays() {
        let (before, after) = head();
        let field = Field::new(&before, &after);

        assert_eq!(field.at([0.0, 0.0, 0.5]), [0.0; 3]);
    }

    #[test]
    fn the_move_has_no_step_where_head_vertices_cross_the_range() {
        let before: Vec<[f32; 3]> = (0..200_u16)
            .map(|index| [0.0, 0.0, f32::from(index) * 0.0001])
            .collect();
        let after: Vec<[f32; 3]> = before.iter().map(|[x, y, z]| [*x, *y, z + 0.05]).collect();
        let field = Field::new(&before, &after);
        let edge = 0.0199 + super::RANGE;

        let inside = field.at([0.0, 0.0, edge - 0.0002])[2];
        let outside = field.at([0.0, 0.0, edge + 0.0002])[2];

        assert!((inside - outside).abs() < 1e-3, "{inside} {outside}");
    }

    #[test]
    fn the_move_fades_with_the_distance() {
        let (before, after) = head();
        let field = Field::new(&before, &after);

        let near = field.at([0.0, 0.0, 0.01])[2];
        let farther = field.at([0.0, 0.0, 0.06])[2];

        assert!(near > farther && farther > 0.0, "{near} {farther}");
    }
}
