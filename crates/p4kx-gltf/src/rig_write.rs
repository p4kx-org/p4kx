// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use gltf_json as json;
use p4kx_model::{Bone, Influences};

use super::{RigPiece, document, instances_of, plan_animations, remap};
use crate::GltfError;
use crate::animations::push_animations;
use crate::glb::{GlbOptions, GlbSummary, MaterialMode, write_file};
use crate::glb_plan::{summarise, texture_set};
use crate::layout::{Layout, Rigging};
use crate::rig_animation::RigAnimation;
use crate::rig_emission::RiggedGlbEmission;
use crate::skin::inverse_binds;
use crate::texture_loader::TextureLoader;
use crate::texture_plan::TextureSet;

/// Writes `pieces` worn on `skeleton` with `animations` as one GLB file to `out`, taking the
/// textures from `textures`; see [`super::write_rigged_glb_with`].
///
/// Animations with keys move the bone nodes by controller ID like
/// [`write_glb_animated`](crate::write_glb_animated) does; sampled animations move the bones they
/// name and set the morph weights of the pieces they name, with the same `LINEAR` or, for a single
/// sample, `STEP` interpolation. Animations without a channel are not written and every
/// animation asked for is listed in [`GlbSummary::animations`].
///
/// # Errors
///
/// As [`super::write_rigged_glb`].
pub fn write_rigged_glb_animated_with<'m>(
    skeleton: &'m [Bone],
    pieces: &'m [RigPiece<'m>],
    animations: &[RigAnimation<'m>],
    options: &GlbOptions,
    textures: &dyn TextureLoader,
    out: &mut impl Write,
) -> Result<GlbSummary, GltfError> {
    write_rigged(skeleton, pieces, animations, options, textures, out, None)
}

/// Writes the rigged GLB and returns which part geometry and material slots it used.
///
/// # Errors
/// As [`write_rigged_glb_animated_with`].
pub fn write_rigged_glb_animated_observed<'m>(
    skeleton: &'m [Bone],
    pieces: &'m [RigPiece<'m>],
    animations: &[RigAnimation<'m>],
    options: &GlbOptions,
    textures: &dyn TextureLoader,
    out: &mut impl Write,
) -> Result<(GlbSummary, RiggedGlbEmission), GltfError> {
    let mut emission = RiggedGlbEmission::default();
    let summary = write_rigged(
        skeleton,
        pieces,
        animations,
        options,
        textures,
        out,
        Some(&mut emission),
    )?;
    Ok((summary, emission))
}

fn write_rigged<'m>(
    skeleton: &'m [Bone],
    pieces: &'m [RigPiece<'m>],
    animations: &[RigAnimation<'m>],
    options: &GlbOptions,
    textures: &dyn TextureLoader,
    out: &mut impl Write,
    emission: Option<&mut RiggedGlbEmission>,
) -> Result<GlbSummary, GltfError> {
    let (instances, fallbacks) = instances_of(pieces, options);
    let binds = inverse_binds(skeleton);
    let remapped: Vec<Vec<Vec<Influences>>> = instances
        .iter()
        .map(|instance| remap(skeleton, instance))
        .collect();
    let mut layout = Layout::new().limited(options.max_influences);
    for ((part, instance), own) in instances.iter().enumerate().zip(&remapped) {
        if let Some(lod) = instance.lod {
            let rigging = Rigging {
                binds: (!binds.is_empty()).then_some(binds.as_slice()),
                influences: Some(own.as_slice()),
                morphs: pieces.get(part).map_or(&[], |piece| piece.morphs),
            };
            layout.add_part(part, lod, options.vertex_colours, &instance.plan, rigging);
        }
    }
    let drawn_nodes: usize = instances
        .iter()
        .map(|instance| instance.model.nodes.len())
        .sum();
    let plans = plan_animations(skeleton, pieces, animations, &mut layout);
    let mut images = match options.materials {
        MaterialMode::Textures => texture_set(&layout, &instances, options.texture_mip, textures),
        MaterialMode::Colours => TextureSet::default(),
    };
    layout.push_images(images.images.iter().map(|image| image.png.len() as u64));
    let mut root = document(skeleton, pieces, &instances, &layout, &images);
    if let Some(buffer) = root.buffers.first().map(|_| json::Index::new(0)) {
        push_animations(&mut root, buffer, &layout, &plans);
    }
    let total = write_file(&root, &layout, &images, out)?;
    if let Some(emission) = emission {
        *emission = RiggedGlbEmission::of(&instances, &layout, &mut images);
    }
    let defaulted = instances
        .iter()
        .map(|instance| instance.plan.defaulted())
        .sum();
    let mut summary = summarise(
        &root,
        &layout,
        defaulted,
        drawn_nodes,
        images,
        &plans,
        total,
    );
    summary.parts = instances.len();
    summary.lod_fallbacks = fallbacks;
    Ok(summary)
}
