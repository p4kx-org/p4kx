// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};

use anyhow::Result;
use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, EntityNode, ForgePalettes,
    LoadoutChanges, LoadoutResolver,
};
use p4kx_blender::{
    BuildInfo, Issue, PackageInfo, PackageSummary, Replacement, check_output, write_package,
};
use p4kx_channels::{PublicPatches, channel_name};
use p4kx_extraction::{Archive, DataForgeEntry, GlbOptions, is_same_file};
use serde::Serialize;

use crate::cli_entity::EntityPackageArgs;
use crate::entity_glb::GlbOutputIsArchive;
use crate::entity_json::is_zero;
use crate::extract::cancel_on_interrupt;
use crate::json;
use crate::query_table::one_line;

#[derive(Serialize)]
struct PackageJson<'a> {
    archive: String,
    entity: &'a str,
    guid: String,
    out: String,
    lod: u32,
    texture_mip: u32,
    nodes: usize,
    drawn_nodes: usize,
    containers: usize,
    objects: usize,
    lights: usize,
    meshes: usize,
    shared_meshes: usize,
    textures: usize,
    texture_pixels: u64,
    texture_failures: usize,
    materials: usize,
    palettes: usize,
    files: usize,
    bytes: u64,
    vertices: usize,
    triangles: usize,
    lod_fallbacks: usize,
    #[serde(skip_serializing_if = "is_zero")]
    bones: usize,
    #[serde(skip_serializing_if = "is_zero")]
    skinned_meshes: usize,
    skipped: usize,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    ignored: &'a [String],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    incompatible: Vec<crate::entity_ignored::IncompatibleJson<'a>>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    excluded: &'a [String],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    replaced: Vec<ReplacedJson<'a>>,
    issues: &'a [Issue],
}

#[derive(Serialize)]
struct ReplacedJson<'a> {
    port: &'a str,
    entity: &'a str,
}

/// Runs `p4kx entity package`.
pub(crate) fn run(args: &EntityPackageArgs, as_json: bool) -> Result<()> {
    if is_same_file(&args.out, &args.archive) {
        return Err(GlbOutputIsArchive(args.out.clone()).into());
    }
    check_output(&args.out)?;
    let changes = args.changes.changes()?;
    let archive = crate::key::open(&args.archive)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let record = resolver.entities().find(&args.entity)?;
    let applied = resolver.resolve_changed(&record, &changes)?;
    crate::entity_ignored::warn(&applied)?;
    let tree = applied.node();
    let models = ArchiveModels::new(&archive);
    let palettes = ForgePalettes::new(&forge);
    let options = AssemblyOptions::default().with_lod(args.lod);
    let assembly = Assembly::build_with_palettes(tree, &models, &palettes, &options)?;
    crate::entity_root::check(&assembly)?;
    let assembly = if args.interior {
        crate::interior_forge::with_interior(assembly, (&archive, &forge), tree, args.lod)?
    } else {
        assembly
    };
    let cancel = cancel_on_interrupt()?;
    let mut glb = GlbOptions::default()
        .with_lod(args.lod)
        .with_texture_mip(args.texture_mip);
    if args.no_vertex_colours {
        glb = glb.without_vertex_colours();
    }
    let info = info_of(&archive, &changes);
    let loader = models.textures();
    let summary = write_package(&assembly, &models, &loader, &glb, &info, &args.out, &|| {
        cancel.is_cancelled()
    })?;
    if as_json {
        json::print(&PackageJson::of(
            &json::path(archive.path()),
            tree,
            args,
            (&info, &summary),
            &applied,
        ))?;
        return Ok(());
    }
    match print_report(&summary, args, &info, tree.class_name()) {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(()),
    }
}

impl<'a> PackageJson<'a> {
    fn of(
        archive: &str,
        tree: &'a EntityNode,
        args: &EntityPackageArgs,
        (info, summary): (&'a PackageInfo, &'a PackageSummary),
        applied: &'a p4kx_assembly::ChangedLoadout,
    ) -> Self {
        Self {
            archive: archive.to_owned(),
            entity: tree.class_name(),
            guid: tree.guid().to_string(),
            out: json::path(&args.out),
            lod: args.lod,
            texture_mip: args.texture_mip,
            nodes: summary.nodes,
            drawn_nodes: summary.drawn_nodes,
            containers: summary.containers,
            objects: summary.objects,
            lights: summary.lights,
            meshes: summary.meshes,
            shared_meshes: summary.shared_meshes,
            textures: summary.textures,
            texture_pixels: summary.texture_pixels,
            texture_failures: summary.texture_failures,
            materials: summary.materials,
            palettes: summary.palettes,
            files: summary.files,
            bytes: summary.bytes,
            vertices: summary.vertices,
            triangles: summary.triangles,
            lod_fallbacks: summary.lod_fallbacks,
            bones: summary.bones,
            skinned_meshes: summary.skinned_meshes,
            skipped: summary.issues.iter().filter(|issue| issue.skipped).count(),
            ignored: applied.ignored(),
            incompatible: crate::entity_ignored::incompatible(applied),
            excluded: &info.exclude,
            replaced: info
                .replace
                .iter()
                .map(|replacement| ReplacedJson {
                    port: &replacement.port,
                    entity: &replacement.entity,
                })
                .collect(),
            issues: &summary.issues,
        }
    }
}

/// Returns the Build of `archive` as a package records it.
pub(crate) fn build_info(archive: &Archive) -> Option<BuildInfo> {
    let build = archive.build()?;
    let patches = PublicPatches::standard();
    Some(BuildInfo {
        branch: build.branch().to_owned(),
        version: build.version().to_owned(),
        public_patch: patches.of(build).map(str::to_owned),
        change_number: build.change_number().to_owned(),
        build_id: build.build_id().to_owned(),
    })
}

fn info_of(archive: &Archive, changes: &LoadoutChanges) -> PackageInfo {
    let build = build_info(archive);
    let replace = changes
        .replacements()
        .iter()
        .map(|(port, entity)| Replacement {
            port: port.clone(),
            entity: entity.clone(),
        })
        .collect();
    PackageInfo::new(channel_name(archive.path()), build)
        .with_changes(changes.exclusions().to_vec(), replace)
}

fn print_changes(out: &mut impl Write, info: &PackageInfo) -> io::Result<()> {
    for port in &info.exclude {
        writeln!(out, "excluded {}", one_line(port))?;
    }
    for replacement in &info.replace {
        writeln!(
            out,
            "replaced {} by {}",
            one_line(&replacement.port),
            one_line(&replacement.entity)
        )?;
    }
    Ok(())
}

fn print_report(
    summary: &PackageSummary,
    args: &EntityPackageArgs,
    info: &PackageInfo,
    class_name: &str,
) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    writeln!(
        out,
        "wrote {} of {}: {} nodes ({} drawn), {} meshes, {} materials, {} palettes, {} textures, {} files, {} vertices, {} triangles, {} bytes",
        args.out.display(),
        one_line(class_name),
        summary.nodes,
        summary.drawn_nodes,
        summary.meshes,
        summary.materials,
        summary.palettes,
        summary.textures,
        summary.files,
        summary.vertices,
        summary.triangles,
        summary.bytes
    )?;
    print_changes(&mut out, info)?;
    if args.interior {
        writeln!(
            out,
            "Interior: {} object containers, {} objects, {} lights",
            summary.containers, summary.objects, summary.lights
        )?;
    }
    if summary.bones > 0 {
        writeln!(
            out,
            "rig in one file: {} bones, skinned meshes {}",
            summary.bones, summary.skinned_meshes
        )?;
    }
    if summary.shared_meshes > 0 {
        writeln!(
            out,
            "{} nodes draw a mesh another node draws; {} meshes are written once",
            summary.shared_meshes, summary.meshes
        )?;
    }
    if summary.lod_fallbacks > 0 {
        writeln!(
            out,
            "{} models lack LOD{} and are written at the nearest lower level",
            summary.lod_fallbacks, args.lod
        )?;
    }
    if summary.texture_failures > 0 {
        writeln!(
            out,
            "{} textures could not be written; their materials keep the plain colours",
            summary.texture_failures
        )?;
    }
    print_issues(&mut out, &summary.issues)?;
    out.flush()
}

fn print_issues(out: &mut impl Write, issues: &[Issue]) -> io::Result<()> {
    for issue in issues {
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
    Ok(())
}
