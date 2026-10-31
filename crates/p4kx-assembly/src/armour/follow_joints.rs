// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_model::{Bone, Influences, MAX_INFLUENCES, Mesh, Model, extend_skeleton};
use rayon::prelude::*;

use super::facial::bones_of;
use super::follow::{ANCHOR, Grid, Vec3};

/// The sum of the weights of a vertex, as the bytes of a glTF.
const BYTE_SUM: i32 = 255;

/// The bones of the head's skin and the bones each vertex of its first mesh is weighted to.
pub(super) struct HeadSkin<'a> {
    bones: &'a [Bone],
    influences: &'a [Influences],
}

impl<'a> HeadSkin<'a> {
    /// Returns the skin of `mesh`, the first mesh of `model`; `None` when the mesh is not skinned.
    pub(super) fn of(model: &'a Model, mesh: &'a Mesh) -> Option<Self> {
        let influences = mesh
            .influences
            .as_deref()
            .filter(|list| list.len() == mesh.positions.len())?;
        Some(Self {
            bones: bones_of(model),
            influences,
        })
    }
}

/// The weight each bone has so far for the vertex being bound.
struct Scratch {
    sums: Vec<f32>,
    touched: Vec<u16>,
}

impl Scratch {
    fn new(bones: usize) -> Self {
        Self {
            sums: vec![0.0; bones],
            touched: Vec::new(),
        }
    }

    /// Adds `scale` times the weights of `influences`, as fractions of their sum, to the bones
    /// that `map` gives; returns whether any weight was added.
    fn add(
        &mut self,
        influences: &Influences,
        scale: f32,
        map: impl Fn(u16) -> Option<u16>,
    ) -> bool {
        let total = f32::from(u16::try_from(influences.sum()).unwrap_or(u16::MAX));
        if total <= 0.0 || scale <= 0.0 {
            return false;
        }
        let mut added = false;
        for (&joint, &weight) in influences.joints.iter().zip(&influences.weights) {
            let Some(joint) = map(joint).filter(|_| weight > 0) else {
                continue;
            };
            let Some(sum) = self.sums.get_mut(usize::from(joint)) else {
                continue;
            };
            if *sum == 0.0 {
                self.touched.push(joint);
            }
            *sum += scale * f32::from(weight) / total;
            added = true;
        }
        added
    }

    /// Returns the strongest [`MAX_INFLUENCES`] bones with their weights as bytes that add up to
    /// 255, and clears the sums; ties go to the lower bone.
    fn finish(&mut self) -> Option<Influences> {
        let Self { sums, touched } = self;
        let weight_of = |joint: u16| sums.get(usize::from(joint)).copied().unwrap_or(0.0);
        touched.sort_unstable_by(|&a, &b| weight_of(b).total_cmp(&weight_of(a)).then(a.cmp(&b)));
        let total: f32 = touched
            .iter()
            .take(MAX_INFLUENCES)
            .map(|&j| weight_of(j))
            .sum();
        let mut pairs = [(0_u16, 0_u8); MAX_INFLUENCES];
        if total.is_finite() && total > 0.0 {
            for (pair, &joint) in pairs.iter_mut().zip(touched.iter()) {
                *pair = (joint, to_byte(weight_of(joint) / total));
            }
        }
        for &joint in touched.iter() {
            if let Some(sum) = sums.get_mut(usize::from(joint)) {
                *sum = 0.0;
            }
        }
        touched.clear();
        let given: i32 = pairs.iter().map(|&(_, byte)| i32::from(byte)).sum();
        let first = pairs.first_mut().filter(|first| first.1 > 0)?;
        let corrected = (i32::from(first.1) + BYTE_SUM - given).clamp(1, BYTE_SUM);
        first.1 = u8::try_from(corrected).unwrap_or(u8::MAX);
        Some(Influences::from_pairs(pairs.into_iter()))
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the fraction is clamped to 0..=1 and scaled to a byte"
)]
fn to_byte(fraction: f32) -> u8 {
    (fraction.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Returns the bones `own` is weighted to for a vertex at `position`, blended with those of the
/// head's vertices around it: the weights of the head vertices are those of the follow kernel,
/// against the vertex's own bones as the anchor. `None` when no head vertex is near.
fn blended(
    position: Vec3,
    own: &Influences,
    (grid, skin, map): (&Grid<'_>, &HeadSkin<'_>, &[Option<u16>]),
    scratch: &mut Scratch,
) -> Option<Influences> {
    scratch.add(own, ANCHOR, Some);
    let mut near = false;
    grid.visit(position, |index, weight| {
        if let Some(head) = skin.influences.get(index) {
            let of_head = |joint: u16| map.get(usize::from(joint)).copied().flatten();
            near |= scratch.add(head, weight, of_head);
        }
    });
    let blended = scratch.finish();
    blended.filter(|_| near)
}

/// Binds the vertices of the first level of `model`, which follows the head whose vertices `grid`
/// holds, to the joints of the head: each vertex takes the bones of the head vertices around it,
/// weighted as the neutral shape follows the head (the hair's own bones count as the anchor), and
/// the bones of the head that the model lacks join its skeleton.
///
/// The result is deterministic; a vertex has at most [`MAX_INFLUENCES`] bones. A mesh that is not
/// skinned stays as it is.
pub(super) fn bind(model: &mut Model, grid: &Grid<'_>, skin: &HeadSkin<'_>) {
    let Some(lod) = model.lods.first() else {
        return;
    };
    let merged = extend_skeleton(model.bones_of(lod), skin.bones, &[]);
    if u16::try_from(merged.len()).is_err() {
        return;
    }
    let map = joint_map(&merged, skin.bones);
    let Some(lod) = model.lods.first_mut() else {
        return;
    };
    let mut bound = false;
    for mesh in &mut lod.meshes {
        bound |= bind_mesh(mesh, (grid, skin, &map), merged.len());
    }
    if !bound {
        return;
    }
    match &mut lod.bones {
        Some(bones) => *bones = merged,
        None => model.bones = merged,
    }
}

/// Returns, for each bone of `head`, the bone of `bones` of the same name, ignoring ASCII case.
fn joint_map(bones: &[Bone], head: &[Bone]) -> Vec<Option<u16>> {
    let mut by_name: HashMap<String, u16> = HashMap::with_capacity(bones.len());
    for (index, bone) in bones.iter().enumerate() {
        if let Ok(index) = u16::try_from(index) {
            by_name
                .entry(bone.name.to_ascii_lowercase())
                .or_insert(index);
        }
    }
    head.iter()
        .map(|bone| by_name.get(&bone.name.to_ascii_lowercase()).copied())
        .collect()
}

/// Binds the vertices of `mesh`, in parallel; returns whether the mesh is skinned.
fn bind_mesh(
    mesh: &mut Mesh,
    context: (&Grid<'_>, &HeadSkin<'_>, &[Option<u16>]),
    bones: usize,
) -> bool {
    let Some(own) = mesh
        .influences
        .as_mut()
        .filter(|list| list.len() == mesh.positions.len())
    else {
        return false;
    };
    own.par_iter_mut().zip(&mesh.positions).for_each_init(
        || Scratch::new(bones),
        |scratch, (own, &position)| {
            if let Some(bound) = blended(position, own, context, scratch) {
                *own = bound;
            }
        },
    );
    true
}

#[cfg(test)]
mod tests {
    use p4kx_model::Influences;

    use super::{Grid, HeadSkin, Scratch, blended};

    const JAW: u16 = 2;

    fn skin(influences: &[Influences]) -> HeadSkin<'_> {
        HeadSkin {
            bones: &[],
            influences,
        }
    }

    fn only(joint: u16) -> Influences {
        Influences::from_pairs([(joint, 255)].into_iter())
    }

    fn weight_of(influences: &Influences, joint: u16) -> u8 {
        influences
            .joints
            .iter()
            .zip(&influences.weights)
            .find(|&(&bone, &weight)| bone == joint && weight > 0)
            .map_or(0, |(_, &weight)| weight)
    }

    #[test]
    fn a_vertex_on_the_head_takes_the_bone_of_the_head_vertex() {
        let head = [[0.0, 0.0, 0.0]];
        let weights = [only(JAW)];
        let grid = Grid::new(&head);
        let map = [None, None, Some(JAW)];
        let mut scratch = Scratch::new(4);

        let bound = blended(
            [0.0, 0.0, 0.001],
            &only(1),
            (&grid, &skin(&weights), &map),
            &mut scratch,
        );

        let bound = bound.expect("a head vertex is near");
        assert!(weight_of(&bound, JAW) > 250, "{bound:?}");
        assert_eq!(bound.sum(), 255);
    }

    #[test]
    fn a_vertex_far_from_the_head_keeps_its_bones() {
        let head = [[0.0, 0.0, 0.0]];
        let weights = [only(JAW)];
        let grid = Grid::new(&head);
        let map = [None, None, Some(JAW)];
        let mut scratch = Scratch::new(4);

        let bound = blended(
            [0.0, 0.0, 0.5],
            &only(1),
            (&grid, &skin(&weights), &map),
            &mut scratch,
        );

        assert_eq!(bound, None);
    }

    #[test]
    fn the_own_bones_weigh_as_much_as_the_anchor_against_the_head() {
        let head = [[0.0, 0.0, 0.0]];
        let weights = [only(JAW)];
        let grid = Grid::new(&head);
        let map = [None, None, Some(JAW)];
        let mut scratch = Scratch::new(4);
        let mut near = |height: f32| {
            blended(
                [0.0, 0.0, height],
                &only(1),
                (&grid, &skin(&weights), &map),
                &mut scratch,
            )
            .map(|bound| weight_of(&bound, 1))
        };

        let (close, far) = (near(0.01), near(0.09));

        assert!(close < Some(30) && far > Some(100), "{close:?} {far:?}");
    }

    #[test]
    fn the_bones_are_the_strongest_eight_and_the_weights_add_up_to_255() {
        let head: Vec<[f32; 3]> = (0..12_u8)
            .map(|i| [0.0, 0.0, f32::from(i) * 0.001])
            .collect();
        let weights: Vec<Influences> = (0..12_u16).map(only).collect();
        let grid = Grid::new(&head);
        let map: Vec<Option<u16>> = (0..12).map(Some).collect();
        let mut scratch = Scratch::new(12);

        let bound = blended(
            [0.0, 0.0, 0.0],
            &only(11),
            (&grid, &skin(&weights), &map),
            &mut scratch,
        )
        .expect("head vertices are near");

        assert_eq!(bound.count(), 8);
        assert_eq!(bound.sum(), 255);
    }

    #[test]
    fn a_vertex_is_bound_the_same_every_time() {
        let head: Vec<[f32; 3]> = (0..6_u8).map(|i| [f32::from(i) * 0.01, 0.0, 0.0]).collect();
        let weights: Vec<Influences> = (0..6_u16).map(|i| only(i % 3)).collect();
        let grid = Grid::new(&head);
        let map: Vec<Option<u16>> = (0..3).map(Some).collect();
        let mut scratch = Scratch::new(3);
        let mut run = || {
            blended(
                [0.02, 0.01, 0.0],
                &only(1),
                (&grid, &skin(&weights), &map),
                &mut scratch,
            )
        };

        assert_eq!(run(), run());
    }

    #[test]
    fn a_head_bone_without_a_target_is_left_out() {
        let head = [[0.0, 0.0, 0.0]];
        let weights = [only(0)];
        let grid = Grid::new(&head);
        let mut scratch = Scratch::new(4);

        let bound = blended(
            [0.0, 0.0, 0.0],
            &only(1),
            (&grid, &skin(&weights), &[None]),
            &mut scratch,
        );

        assert_eq!(bound, None);
    }
}
