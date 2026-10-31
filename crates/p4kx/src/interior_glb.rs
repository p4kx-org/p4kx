// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use p4kx_assembly::{Interior, InteriorOptions, Issue};
use p4kx_extraction::{
    Archive, ConversionError, GlbOptions, GlbSummary, GltfError, MaterialMode, UnknownEntries,
    is_same_file, select_entries, write_replacing,
};
use serde::Serialize;
use thiserror::Error;

use crate::cli_interior::{InteriorGlbArgs, InteriorMaterialsArg};
use crate::entity_glb_json::IssueJson;
use crate::json;
use crate::layers_json::LayersJson;
use crate::query_table::one_line;

/// The output of `p4kx interior glb` is the Archive it reads.
#[derive(Debug, Error)]
#[error("{} is the Archive the Interior is read from", .0.display())]
pub(crate) struct InteriorOutputIsArchive(pub(crate) PathBuf);

#[derive(Serialize)]
struct GlbJson<'a> {
    archive: String,
    containers: &'a [String],
    out: String,
    lod: u32,
    parts: usize,
    containers_opened: usize,
    model_parts: usize,
    modelled_parts: usize,
    props: usize,
    lights: usize,
    nodes: usize,
    meshes: usize,
    primitives: usize,
    materials: usize,
    default_material_ids: usize,
    hidden_primitives: usize,
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
    issues: Vec<IssueJson<'a>>,
}

/// Opens the Archive and every container of the chain in turn; returns all of them, outermost
/// first, and the Entry paths of the chain as the Archive and the containers store them.
pub(crate) fn open(archive: &Path, containers: &[String]) -> Result<(Vec<Archive>, Vec<String>)> {
    let mut layers = vec![crate::key::open(archive)?];
    let mut chain = Vec::with_capacity(containers.len());
    for path in containers {
        let Some(current) = layers.last() else {
            break;
        };
        let found = select_entries(current, &[path])?;
        let Some(entry) = found.first() else {
            return Err(UnknownEntries(vec![path.clone()]).into());
        };
        chain.push(entry.path().to_owned());
        let next = current.open_container(entry)?;
        layers.push(next);
    }
    Ok((layers, chain))
}

/// Runs `p4kx interior glb`.
pub(crate) fn run(args: &InteriorGlbArgs, as_json: bool) -> Result<()> {
    if is_same_file(&args.out, &args.archive) {
        return Err(InteriorOutputIsArchive(args.out.clone()).into());
    }
    let (layers, chain) = open(&args.archive, &args.containers)?;
    let entry = chain.last().map_or("", String::as_str);
    let options = InteriorOptions::default().with_lod(args.lod);
    let interior = crate::interior_forge::build(&layers, entry, &options)?;
    let summary = write(&interior, args, entry)?;
    let skipped = interior
        .issues()
        .iter()
        .filter(|issue| issue.kind().skips())
        .count();
    if as_json {
        json::print(&GlbJson {
            archive: json::path(&args.archive),
            containers: &chain,
            out: json::path(&args.out),
            lod: args.lod,
            parts: interior.parts().len(),
            containers_opened: interior.containers(),
            model_parts: interior.model_parts(),
            modelled_parts: interior.modelled_parts(),
            props: interior.props(),
            lights: summary.lights,
            nodes: summary.nodes,
            meshes: summary.meshes,
            primitives: summary.primitives,
            materials: summary.materials,
            default_material_ids: summary.default_material_ids,
            hidden_primitives: summary.hidden_primitives,
            lod_fallbacks: summary.lod_fallbacks,
            shared_meshes: summary.shared_meshes,
            shared_materials: summary.shared_materials,
            shared_textures: summary.shared_textures,
            vertices: summary.vertices,
            triangles: summary.triangles,
            bytes: summary.bytes,
            skipped,
            layers: LayersJson::from(&interior.unresolved_layers()),
            issues: interior.issues().iter().map(IssueJson::from).collect(),
        })?;
        return Ok(());
    }
    match print_report(&interior, &summary, &args.out, entry) {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(()),
    }
}

fn write(interior: &Interior, args: &InteriorGlbArgs, entry: &str) -> Result<GlbSummary> {
    let mode = match args.materials {
        InteriorMaterialsArg::Colours => MaterialMode::Colours,
    };
    let mut options = GlbOptions::default()
        .with_lod(args.lod)
        .with_materials(mode);
    if args.no_vertex_colours {
        options = options.without_vertex_colours();
    }
    let out: &Path = &args.out;
    Ok(write_replacing(out, |writer| {
        interior
            .write_glb(&options, writer)
            .map_err(|error| match error {
                GltfError::Write(source) => ConversionError::Write {
                    path: out.to_path_buf(),
                    source,
                },
                source => ConversionError::Glb {
                    file: entry.to_owned(),
                    source,
                },
            })
    })?)
}

fn print_issue(out: &mut impl Write, issue: &Issue) -> io::Result<()> {
    let what = if issue.kind().skips() {
        "skipped"
    } else {
        "note"
    };
    let path = if issue.path().is_empty() {
        issue.name().to_owned()
    } else {
        format!("{} ({})", issue.path(), issue.name())
    };
    writeln!(
        out,
        "{what} {}: {}",
        one_line(&path),
        one_line(&issue.kind().to_string())
    )
}

fn print_report(
    interior: &Interior,
    summary: &GlbSummary,
    out: &Path,
    entry: &str,
) -> io::Result<()> {
    let mut stdout = BufWriter::new(io::stdout().lock());
    writeln!(
        stdout,
        "wrote {} of {}: {} parts ({} models of which {} props, {} with geometry, {} lights) in {} containers, {} nodes, {} meshes, {} primitives, {} materials, {} vertices, {} triangles, {} bytes",
        out.display(),
        one_line(entry),
        interior.parts().len(),
        interior.model_parts(),
        interior.props(),
        interior.modelled_parts(),
        summary.lights,
        interior.containers(),
        summary.nodes,
        summary.meshes,
        summary.primitives,
        summary.materials,
        summary.vertices,
        summary.triangles,
        summary.bytes
    )?;
    if summary.lod_fallbacks > 0 {
        writeln!(
            stdout,
            "{} models lack LOD{} and are written at the nearest lower level",
            summary.lod_fallbacks,
            interior.lod()
        )?;
    }
    if summary.shared_meshes + summary.shared_materials + summary.shared_textures > 0 {
        writeln!(
            stdout,
            "shared {} meshes, {} materials and {} textures instead of writing them again",
            summary.shared_meshes, summary.shared_materials, summary.shared_textures
        )?;
    }
    if summary.hidden_primitives > 0 {
        writeln!(
            stdout,
            "left out {} submeshes whose material draws nothing",
            summary.hidden_primitives
        )?;
    }
    for issue in interior.issues() {
        print_issue(&mut stdout, issue)?;
    }
    stdout.flush()
}
