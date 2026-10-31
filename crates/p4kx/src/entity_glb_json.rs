// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The JSON report of `p4kx entity glb`.

use std::path::Path;

use anyhow::Result;
use p4kx_assembly::{Assembly, ChangedLoadout, EntityNode, Issue, LoadoutChanges};
use p4kx_extraction::GlbSummary;
use serde::Serialize;

use crate::cli_entity::EntityGlbArgs;
use crate::entity_glb_body::{BodyJson, body_json};
use crate::entity_glb_placements::{PlacementJson, painted, placements_json};
use crate::json;
use crate::layers_json::LayersJson;

/// What the JSON report of one `entity glb` run is built from.
pub(crate) struct GlbReport<'a> {
    pub(crate) archive: &'a Path,
    pub(crate) args: &'a EntityGlbArgs,
    pub(crate) tree: &'a EntityNode,
    pub(crate) assembly: &'a Assembly,
    pub(crate) summary: &'a GlbSummary,
    pub(crate) applied: &'a ChangedLoadout,
    pub(crate) changes: &'a LoadoutChanges,
}

impl GlbReport<'_> {
    /// Prints the report to stdout.
    pub(crate) fn print(&self) -> Result<()> {
        let Self {
            archive,
            args,
            tree,
            assembly,
            summary,
            applied,
            changes,
        } = *self;
        Ok(json::print(&GlbJson {
            archive: json::path(archive),
            entity: tree.class_name(),
            guid: tree.guid().to_string(),
            out: json::path(&args.out),
            lod: args.lod,
            parts: assembly.parts().len(),
            modelled_parts: assembly.modelled_parts(),
            hidden_parts: assembly.hidden_parts(),
            palettes: painted(assembly),
            nodes: summary.nodes,
            meshes: summary.meshes,
            primitives: summary.primitives,
            materials: summary.materials,
            textured_materials: summary.textured_materials,
            textures: summary.textures,
            texture_pixels: summary.texture_pixels,
            texture_fallbacks: fallbacks_json(summary),
            default_material_ids: summary.default_material_ids,
            hidden_primitives: summary.hidden_primitives,
            bones: summary.bones,
            skinned_meshes: summary.skinned_meshes,
            lod_fallbacks: summary.lod_fallbacks,
            shared_meshes: summary.shared_meshes,
            shared_materials: summary.shared_materials,
            shared_textures: summary.shared_textures,
            vertices: summary.vertices,
            triangles: summary.triangles,
            bytes: summary.bytes,
            skipped: assembly
                .issues()
                .iter()
                .filter(|issue| issue.kind().skips())
                .count(),
            layers: LayersJson::from(&assembly.unresolved_layers()),
            interior: interior_json(assembly),
            body: body_json(tree),
            ignored: applied.ignored(),
            incompatible: crate::entity_ignored::incompatible(applied),
            excluded: changes.exclusions().iter().map(String::as_str).collect(),
            replaced: changes
                .replacements()
                .iter()
                .map(|(port, entity)| ReplacedJson { port, entity })
                .collect(),
            placements: placements_json(assembly),
            issues: assembly.issues().iter().map(IssueJson::from).collect(),
        })?)
    }
}

#[derive(Serialize)]
struct GlbJson<'a> {
    archive: String,
    entity: &'a str,
    guid: String,
    out: String,
    lod: u32,
    parts: usize,
    modelled_parts: usize,
    hidden_parts: usize,
    palettes: usize,
    nodes: usize,
    meshes: usize,
    primitives: usize,
    materials: usize,
    textured_materials: usize,
    textures: usize,
    texture_pixels: u64,
    texture_fallbacks: Vec<FallbackJson<'a>>,
    default_material_ids: usize,
    hidden_primitives: usize,
    bones: usize,
    skinned_meshes: usize,
    lod_fallbacks: usize,
    shared_meshes: usize,
    shared_materials: usize,
    shared_textures: usize,
    vertices: usize,
    triangles: usize,
    bytes: u64,
    skipped: usize,
    #[serde(flatten)]
    layers: LayersJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    interior: Option<InteriorJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<BodyJson<'a>>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    ignored: &'a [String],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    incompatible: Vec<crate::entity_ignored::IncompatibleJson<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    excluded: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    replaced: Vec<ReplacedJson<'a>>,
    placements: Vec<PlacementJson<'a>>,
    issues: Vec<IssueJson<'a>>,
}

#[derive(Serialize)]
struct InteriorJson {
    containers: usize,
    parts: usize,
    model_parts: usize,
    modelled_parts: usize,
    lights: usize,
}

fn interior_json(assembly: &Assembly) -> Option<InteriorJson> {
    assembly.interior().map(|interior| InteriorJson {
        containers: interior.containers().len(),
        parts: interior.parts(),
        model_parts: interior.model_parts(),
        modelled_parts: interior.modelled_parts(),
        lights: interior.lights(),
    })
}

#[derive(Serialize)]
struct FallbackJson<'a> {
    material: &'a str,
    role: &'static str,
    file: &'a str,
    reason: &'a str,
}

#[derive(Serialize)]
struct ReplacedJson<'a> {
    port: &'a str,
    entity: &'a str,
}

/// One Issue of an Assembly in a JSON report.
#[derive(Serialize)]
pub(crate) struct IssueJson<'a> {
    path: &'a str,
    name: &'a str,
    kind: &'static str,
    skipped: bool,
    reason: String,
}

impl<'a> From<&'a Issue> for IssueJson<'a> {
    fn from(issue: &'a Issue) -> Self {
        Self {
            path: issue.path(),
            name: issue.name(),
            kind: issue.kind().code(),
            skipped: issue.kind().skips(),
            reason: issue.kind().to_string(),
        }
    }
}

fn fallbacks_json(summary: &GlbSummary) -> Vec<FallbackJson<'_>> {
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
