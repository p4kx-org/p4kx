// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use anyhow::Result;
use p4kx_assembly::{Armour, CharacterAnimations, Head};
use p4kx_extraction::{ArchiveDefaultMaterial, DefaultMaterialReceipt, GlbSummary};
use serde::Serialize;

use super::origin::GlbPublication;
use super::origin_code;
use crate::cli::ChfGlbArgs;
use crate::entity_glb_json::IssueJson;
use crate::json;
use crate::model_animations::WrittenJson;

#[derive(Serialize)]
struct PartJson<'a> {
    name: &'a str,
    origin: &'static str,
    file: &'a str,
    morph_targets: usize,
}

#[derive(Serialize)]
struct FallbackJson<'a> {
    material: &'a str,
    role: &'static str,
    file: &'a str,
    reason: &'a str,
}

#[derive(Serialize)]
struct GlbJson<'a> {
    archive: String,
    chf: String,
    body: &'static str,
    rig: &'static str,
    skeleton: &'a str,
    out: String,
    lod: u32,
    parts: Vec<PartJson<'a>>,
    nodes: usize,
    bones: usize,
    meshes: usize,
    skinned_meshes: usize,
    primitives: usize,
    materials: usize,
    morph_targets: usize,
    textured_materials: usize,
    textures: usize,
    texture_pixels: u64,
    texture_fallbacks: Vec<FallbackJson<'a>>,
    default_material_ids: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_material: Option<DefaultMaterialReceipt>,
    vertices: usize,
    triangles: usize,
    bytes: u64,
    skipped: usize,
    animations: Vec<WrittenJson>,
    issues: Vec<IssueJson<'a>>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    publication: Option<GlbPublication>,
}

/// What was built and written.
pub(super) struct Outcome<'a> {
    pub(super) armour: &'a Armour,
    pub(super) animated: &'a CharacterAnimations<'a>,
    pub(super) summary: &'a GlbSummary,
}

fn report<'a>(
    archive: &Path,
    args: &ChfGlbArgs,
    head: &Head,
    outcome: &Outcome<'a>,
) -> GlbJson<'a> {
    let Outcome {
        armour,
        animated,
        summary,
    } = *outcome;
    GlbJson {
        archive: json::path(archive),
        chf: json::path(&args.file),
        body: head.variant().code(),
        rig: head.rig(),
        skeleton: armour.skeleton_file(),
        out: json::path(&args.out),
        lod: args.lod,
        parts: armour
            .parts()
            .iter()
            .map(|part| PartJson {
                name: part.name(),
                origin: origin_code(part.origin()),
                file: part.file(),
                morph_targets: part.morphs().len(),
            })
            .collect(),
        nodes: summary.nodes,
        bones: summary.bones,
        meshes: summary.meshes,
        skinned_meshes: summary.skinned_meshes,
        primitives: summary.primitives,
        materials: summary.materials,
        morph_targets: summary.morph_targets,
        textured_materials: summary.textured_materials,
        textures: summary.textures,
        texture_pixels: summary.texture_pixels,
        texture_fallbacks: texture_fallbacks(summary),
        default_material_ids: summary.default_material_ids,
        default_material: armour
            .default_material(args.lod)
            .map(ArchiveDefaultMaterial::receipt),
        vertices: summary.vertices,
        triangles: summary.triangles,
        bytes: summary.bytes,
        skipped: armour
            .issues()
            .iter()
            .filter(|issue| issue.kind().skips())
            .count(),
        animations: summary.animations.iter().map(WrittenJson::of).collect(),
        issues: armour
            .issues()
            .iter()
            .chain(animated.issues())
            .map(IssueJson::from)
            .collect(),
        publication: None,
    }
}

fn texture_fallbacks(summary: &GlbSummary) -> Vec<FallbackJson<'_>> {
    summary
        .texture_fallbacks
        .iter()
        .map(|fallback| FallbackJson {
            material: &fallback.material,
            role: fallback.role.name(),
            file: &fallback.file,
            reason: &fallback.reason,
        })
        .collect()
}

pub(super) fn print(
    archive: &Path,
    args: &ChfGlbArgs,
    head: &Head,
    outcome: &Outcome<'_>,
    mut publication: GlbPublication,
) -> Result<()> {
    let mut report = report(archive, args, head, outcome);
    publication.cover(&report)?;
    report.publication = Some(publication);
    json::print(&report)?;
    Ok(())
}
