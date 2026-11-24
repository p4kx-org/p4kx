// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::Path;

use anyhow::Result;
use p4kx_assembly::{Armour, BodyVariant, Head, PartOrigin};
use p4kx_blender::{Issue, PackageSummary};
use p4kx_extraction::{AnimationSet, EntryOrigin};
use serde::Serialize;

use crate::armour_loadout::{LoadoutJson, ReplacedJson, Selection};
use crate::cli_character::CharacterPackageArgs;
use crate::json;
use crate::model_animations::{WrittenJson, write_notes};
use crate::query_table::one_line;

/// What the package was made from.
pub(super) struct Shown<'a> {
    pub(super) archive: &'a Path,
    pub(super) args: &'a CharacterPackageArgs,
    pub(super) name: &'a str,
    pub(super) head: Option<&'a Head>,
    pub(super) default_chf: Option<&'a EntryOrigin>,
    pub(super) variant: BodyVariant,
    pub(super) selection: &'a Selection<'a>,
    pub(super) armour: &'a Armour,
    pub(super) found: &'a AnimationSet,
}

#[derive(Serialize)]
struct PartJson<'a> {
    name: &'a str,
    origin: &'static str,
    file: &'a str,
    morph_targets: usize,
}

#[derive(Serialize)]
struct DefaultChfJson<'a> {
    path: &'a str,
    entry_index: usize,
}

#[derive(Serialize)]
struct PackageJson<'a> {
    archive: String,
    name: &'a str,
    chf: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_chf: Option<DefaultChfJson<'a>>,
    body: &'static str,
    rig: Option<&'static str>,
    skeleton: &'a str,
    out: String,
    lod: u32,
    loadout: Option<&'a LoadoutJson<'a>>,
    replaced: Vec<ReplacedJson<'a>>,
    parts: Vec<PartJson<'a>>,
    nodes: usize,
    bones: usize,
    skinned_meshes: usize,
    morph_targets: usize,
    materials: usize,
    embedded_textures: usize,
    vertices: usize,
    triangles: usize,
    files: usize,
    bytes: u64,
    skipped: usize,
    animations: Vec<WrittenJson>,
    issues: &'a [Issue],
}

const fn origin_code(origin: PartOrigin) -> &'static str {
    match origin {
        PartOrigin::Body => "body",
        _ => "piece",
    }
}

/// Prints the report as JSON or text.
pub(super) fn print(shown: &Shown<'_>, summary: &PackageSummary, as_json: bool) -> Result<()> {
    if as_json {
        json::print(&json_of(shown, summary))?;
        return Ok(());
    }
    match text(shown, summary) {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(()),
    }
}

fn json_of<'a>(shown: &'a Shown<'a>, summary: &'a PackageSummary) -> PackageJson<'a> {
    PackageJson {
        archive: json::path(shown.archive),
        name: shown.name,
        chf: shown.args.chf.as_deref().map(json::path),
        default_chf: shown.default_chf.map(|origin| DefaultChfJson {
            path: origin.entry().path(),
            entry_index: origin.entry().index(),
        }),
        body: shown.variant.code(),
        rig: shown.head.map(Head::rig),
        skeleton: shown.armour.skeleton_file(),
        out: json::path(&shown.args.out),
        lod: shown.args.lod,
        loadout: shown.selection.loadout.as_ref(),
        replaced: shown
            .selection
            .replaced
            .iter()
            .map(ReplacedJson::from)
            .collect(),
        parts: shown
            .armour
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
        skinned_meshes: summary.skinned_meshes,
        morph_targets: summary.morph_targets,
        materials: summary.materials,
        embedded_textures: summary.embedded_textures,
        vertices: summary.vertices,
        triangles: summary.triangles,
        files: summary.files,
        bytes: summary.bytes,
        skipped: summary.issues.iter().filter(|issue| issue.skipped).count(),
        animations: summary.clips.iter().map(WrittenJson::of).collect(),
        issues: &summary.issues,
    }
}

fn text(shown: &Shown<'_>, summary: &PackageSummary) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    shown.selection.print(&mut out)?;
    if let Some(origin) = shown.default_chf {
        writeln!(
            out,
            "default CHF {} (Entry {})",
            one_line(origin.entry().path()),
            origin.entry().index()
        )?;
    }
    writeln!(
        out,
        "wrote {} of {} on the {} body: {} parts, {} bones, {} skinned meshes, {} materials, {} morph targets, {} animations, {} vertices, {} triangles, {} files, {} bytes",
        shown.args.out.display(),
        one_line(shown.name),
        shown.variant.code(),
        shown.armour.parts().len(),
        summary.bones,
        summary.skinned_meshes,
        summary.materials,
        summary.morph_targets,
        summary.animations(),
        summary.vertices,
        summary.triangles,
        summary.files,
        summary.bytes
    )?;
    for part in shown.armour.parts() {
        writeln!(
            out,
            "part {}: {}",
            one_line(part.name()),
            one_line(part.file())
        )?;
    }
    write_notes(&mut out, &summary.clips, shown.found)?;
    for issue in &summary.issues {
        let what = if issue.skipped { "skipped" } else { "note" };
        let path = if issue.path.is_empty() {
            issue.name.clone()
        } else {
            format!("{} ({})", issue.path, issue.name)
        };
        writeln!(
            out,
            "{what} {}: {}",
            one_line(&path),
            one_line(&issue.reason)
        )?;
    }
    out.flush()
}
