// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The expression morph targets of a head at level 0 carried to a lower level of detail.
//!
//! The rigs of the lower levels hold no expression data. Their meshes are remeshes of level 0 whose
//! vertices lie on its surface, so each vertex takes the move its nearest point on the surface of
//! level 0 has, interpolated between the corners of that triangle.

use p4kx_gltf::MorphTarget;
use p4kx_model::{Mesh, Model};
use rayon::prelude::*;

use super::anchor::{Anchor, anchors};
use super::expressions::{Expressions, Morphs};
use super::vec3::Vec3;
use crate::{ModelProvider, ProvidedModel};

/// Reads the skin at `path` at level 0, lets `make` turn it into the model as it is worn and its
/// morph targets, and carries the targets to `to`, the mesh of a lower level.
///
/// The surface the targets are carried over is the mesh `make` returns, so a shaped head carries
/// them from its shaped level 0 to the level that was shaped the same way.
pub(super) fn from_level_zero(
    path: &str,
    models: &dyn ModelProvider,
    (to, selection): (&Mesh, &Expressions),
    make: impl FnOnce(Model) -> Result<(Model, Morphs), String>,
) -> Result<Morphs, String> {
    from_level_zero_observed(path, models, (to, selection), |base| {
        make(base.model).map(|(model, morphs)| (model, morphs, ()))
    })
    .map(|(morphs, ())| morphs)
}

pub(super) fn from_level_zero_observed<T>(
    path: &str,
    models: &dyn ModelProvider,
    (to, selection): (&Mesh, &Expressions),
    make: impl FnOnce(ProvidedModel) -> Result<(Model, Morphs, T), String>,
) -> Result<(Morphs, T), String> {
    let base = models
        .model(path, 0)
        .map_err(|failure| format!("{path} cannot be read at level 0: {failure}"))?;
    let (model, morphs, origin) = make(base)?;
    let from = model
        .lods
        .first()
        .and_then(|lod| lod.meshes.first())
        .ok_or_else(|| format!("{path} has no mesh at level 0"))?;
    Ok((carry(morphs, from, to, selection), origin))
}

/// Carries the targets of `morphs`, made on the mesh `from`, to the mesh `to`; a target that moves
/// no vertex of `to` is left out, and listed as empty if `selection` named it.
fn carry(morphs: Morphs, from: &Mesh, to: &Mesh, selection: &Expressions) -> Morphs {
    let anchors = anchors(&from.positions, &from.indices, &to.positions);
    carry_over(morphs, (from.positions.len(), &anchors), selection)
}

/// Carries the targets of `morphs`, made on a mesh of `width` vertices, over `anchors`, the
/// anchors of the vertices of the lower level on it.
fn carry_over(
    morphs: Morphs,
    (width, anchors): (usize, &[Option<Anchor>]),
    selection: &Expressions,
) -> Morphs {
    let made: Vec<Option<MorphTarget>> = morphs
        .targets
        .par_iter()
        .map_init(
            || vec![[0.0_f32; 3]; width],
            |dense, target| carried(dense, target, anchors),
        )
        .collect();
    let named = matches!(selection, Expressions::Only(_));
    let mut empty = morphs.empty;
    let mut targets = Vec::with_capacity(made.len());
    let mut numbers = Vec::with_capacity(made.len());
    for ((target, &number), kept) in morphs.targets.iter().zip(&morphs.numbers).zip(made) {
        match kept {
            Some(kept) => {
                targets.push(kept);
                numbers.push(number);
            }
            None if named => empty.push(target.name.clone()),
            None => {}
        }
    }
    Morphs {
        targets,
        numbers,
        unknown: morphs.unknown,
        empty,
    }
}

/// Returns `target` on the vertices the anchors name. `dense` is all zero on entry and on exit.
fn carried(
    dense: &mut [Vec3],
    target: &MorphTarget,
    anchors: &[Option<Anchor>],
) -> Option<MorphTarget> {
    for (&vertex, &delta) in target.vertices.iter().zip(&target.deltas) {
        if let Some(slot) = dense.get_mut(vertex as usize) {
            *slot = delta;
        }
    }
    let mut vertices = Vec::new();
    let mut deltas = Vec::new();
    for (vertex, anchor) in anchors.iter().enumerate() {
        let (Some(anchor), Ok(vertex)) = (anchor, u32::try_from(vertex)) else {
            continue;
        };
        let mut delta = [0.0_f32; 3];
        for (&corner, &weight) in anchor.corners.iter().zip(&anchor.weights) {
            let Some(moved) = dense.get(corner as usize) else {
                continue;
            };
            for (total, value) in delta.iter_mut().zip(moved) {
                *total = weight.mul_add(*value, *total);
            }
        }
        if delta.iter().any(|value| *value != 0.0) {
            vertices.push(vertex);
            deltas.push(delta);
        }
    }
    for &vertex in &target.vertices {
        if let Some(slot) = dense.get_mut(vertex as usize) {
            *slot = [0.0; 3];
        }
    }
    (!vertices.is_empty()).then(|| MorphTarget {
        name: target.name.clone(),
        vertices,
        deltas,
    })
}

#[cfg(test)]
mod tests {
    use p4kx_gltf::MorphTarget;

    use super::{Anchor, Expressions, Morphs, carried, carry_over};

    fn target() -> MorphTarget {
        MorphTarget {
            name: "blink".to_owned(),
            vertices: vec![0, 2],
            deltas: vec![[0.0, 0.0, 0.002], [0.0, 0.0, 0.004]],
        }
    }

    #[test]
    fn a_vertex_between_the_corners_takes_the_interpolated_move() {
        let anchors = [Some(Anchor {
            corners: [0, 1, 2],
            weights: [0.5, 0.0, 0.5],
        })];
        let mut dense = vec![[0.0; 3]; 3];

        let made = carried(&mut dense, &target(), &anchors).expect("a target");

        assert_eq!(made.vertices, vec![0]);
        assert!(
            (made.deltas[0][2] - 0.003).abs() < 1e-9,
            "{:?}",
            made.deltas
        );
        assert_eq!(dense, vec![[0.0; 3]; 3]);
    }

    #[test]
    fn a_vertex_beside_the_moved_ones_is_not_in_the_target() {
        let anchors = [
            Some(Anchor {
                corners: [1, 1, 1],
                weights: [1.0, 0.0, 0.0],
            }),
            None,
        ];
        let mut dense = vec![[0.0; 3]; 3];

        assert!(carried(&mut dense, &target(), &anchors).is_none());
    }

    fn named(name: &str, vertex: u32) -> MorphTarget {
        MorphTarget {
            name: name.to_owned(),
            vertices: vec![vertex],
            deltas: vec![[0.0, 0.0, 0.001]],
        }
    }

    fn morphs() -> Morphs {
        Morphs {
            targets: vec![named("jaw", 0), named("blink", 1), named("brow", 2)],
            numbers: vec![10, 20, 30],
            unknown: Vec::new(),
            empty: Vec::new(),
        }
    }

    fn only_vertex_two() -> [Option<Anchor>; 1] {
        [Some(Anchor {
            corners: [2, 2, 2],
            weights: [1.0, 0.0, 0.0],
        })]
    }

    #[test]
    fn a_dropped_target_takes_its_number_with_it_and_is_listed_when_named() {
        let selection = Expressions::Only(vec!["jaw".to_owned()]);

        let made = carry_over(morphs(), (3, &only_vertex_two()), &selection);

        assert_eq!(made.numbers, vec![30]);
        assert_eq!(
            made.targets
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>(),
            vec!["brow"]
        );
        assert_eq!(made.empty, vec!["jaw".to_owned(), "blink".to_owned()]);
    }

    #[test]
    fn a_dropped_target_of_all_expressions_is_not_listed() {
        let made = carry_over(morphs(), (3, &only_vertex_two()), &Expressions::All);

        assert_eq!(made.numbers, vec![30]);
        assert_eq!(made.empty, Vec::<String>::new());
    }
}
