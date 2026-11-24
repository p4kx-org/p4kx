// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use p4kx_assembly::animation_choice;
use p4kx_extraction::model::{LodSelection, Model};
use p4kx_extraction::model_info::file_name;
use p4kx_extraction::{
    AnimationSet, ArchiveTextures, ChunkInfo, GlbGeometryPublication, GlbOptions, GlbSummary,
    LibraryInfo, MaterialMode, ModelSource, PathIndex, UnreadInfo,
};
use serde::Serialize;

use crate::cli_model::{MaterialsArg, ModelAction};
use crate::json;
use crate::model::{archive_animations, disk_animations};
use crate::model_animations::{WrittenJson, write_notes as write_animation_notes};
use crate::model_text::printable;

#[derive(Serialize)]
struct GlbJson<'a> {
    model: &'a str,
    out: String,
    #[serde(rename = "originReceipt")]
    origin_receipt: String,
    nodes: usize,
    meshes: usize,
    primitives: usize,
    materials: usize,
    textured_materials: usize,
    textures: usize,
    texture_pixels: u64,
    texture_fallbacks: Vec<FallbackJson<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    textures_note: Option<&'static str>,
    default_material_ids: usize,
    hidden_primitives: usize,
    bones: usize,
    skinned_meshes: usize,
    vertices: usize,
    triangles: usize,
    bytes: u64,
    library: LibraryInfo<'a>,
    animations: Vec<WrittenJson>,
    unread_animations: Vec<UnreadInfo>,
    unknown_chunks: Vec<ChunkInfo>,
}

const NO_ARCHIVE: &str = "textures are read from an Archive; give the model as ARCHIVE ENTRY. The materials keep their plain colours";

#[derive(Serialize)]
struct FallbackJson<'a> {
    material: &'a str,
    role: &'static str,
    file: &'a str,
    reason: &'a str,
}

/// Runs `p4kx model glb`.
pub(crate) fn run(action: &ModelAction, as_json: bool) -> Result<()> {
    let ModelAction::Glb {
        source,
        out,
        lod,
        materials,
        texture_mip,
        no_vertex_colours,
        max_influences,
        animations,
    } = action
    else {
        return Ok(());
    };
    let mode = match materials {
        MaterialsArg::Colours => MaterialMode::Colours,
        MaterialsArg::Textures => MaterialMode::Textures,
    };
    let mut options = GlbOptions::default()
        .with_lod(*lod)
        .with_materials(mode)
        .with_texture_mip(*texture_mip)
        .with_max_influences(usize::from(*max_influences));
    if *no_vertex_colours {
        options = options.without_vertex_colours();
    }
    let lods = LodSelection::Only(*lod);
    let choice = animation_choice(animations);
    if let Some(entry) = &source.entry {
        let archive = crate::key::open(&source.path)?;
        let index = PathIndex::new(&archive);
        let model = ModelSource::from_index_with(&index, entry, lods)?;
        let found = archive_animations(&index, entry, source, choice.as_ref())?;
        let textures = ArchiveTextures::new(&index);
        let written =
            model.write_glb_with_geometry_receipt(&found, &options, out, Some(&textures))?;
        report(&model, &found, &options, &written, out, true, as_json)
    } else {
        let model = ModelSource::from_file_with(&source.path, lods)?;
        let found = disk_animations(source, choice.as_ref())?;
        let written = model.write_glb_with_geometry_receipt(&found, &options, out, None)?;
        report(&model, &found, &options, &written, out, false, as_json)
    }
}

fn report(
    source: &ModelSource,
    animations: &AnimationSet,
    options: &GlbOptions,
    written: &GlbGeometryPublication,
    out: &Path,
    from_archive: bool,
    as_json: bool,
) -> Result<()> {
    let model = source.model();
    let summary = &written.summary;
    if as_json {
        let no_archive = options.materials == MaterialMode::Textures && !from_archive;
        json::print(&glb_json(source, animations, out, written, no_archive)?)?;
        return Ok(());
    }
    let mut stdout = BufWriter::new(io::stdout().lock());
    writeln!(
        stdout,
        "wrote {} of {}: {} nodes, {} meshes, {} primitives, {} materials, {} vertices, {} triangles, {} bytes",
        printable(&out.display().to_string()),
        printable(source.name()),
        summary.nodes,
        summary.meshes,
        summary.primitives,
        summary.materials,
        summary.vertices,
        summary.triangles,
        summary.bytes
    )?;
    if options.materials == MaterialMode::Textures {
        write_textures(&mut stdout, summary, from_archive)?;
    }
    write_notes(&mut stdout, model, summary)?;
    write_animation_notes(&mut stdout, &summary.animations, animations)?;
    stdout.flush()?;
    Ok(())
}

fn write_notes(out: &mut impl Write, model: &Model, summary: &GlbSummary) -> io::Result<()> {
    let stdout = out;
    if summary.bones > 0 {
        writeln!(
            stdout,
            "{} bones, {} skinned meshes",
            summary.bones, summary.skinned_meshes
        )?;
    }
    if let Some(problem) = model.materials.problem() {
        writeln!(
            stdout,
            "{}; the default material is used",
            printable(&problem)
        )?;
    }
    if summary.default_material_ids > 0 && model.materials.problem().is_none() {
        writeln!(
            stdout,
            "{} material IDs have no sub-material in the library; the default material is used",
            summary.default_material_ids
        )?;
    }
    if let Some(library) = model.materials.library()
        && library.unknown_entries > 0
    {
        writeln!(
            stdout,
            "{} entries of the material library are no material; the default material is used for them",
            library.unknown_entries
        )?;
    }
    if let Some(library) = model.materials.library()
        && !library.unresolved_layers.is_empty()
    {
        writeln!(
            stdout,
            "{} layer files of the material library are missing or unreadable; their materials keep the colours of their own file",
            library.unresolved_layers.len()
        )?;
    }
    if summary.hidden_primitives > 0 {
        writeln!(
            stdout,
            "left out {} submeshes whose material draws nothing",
            summary.hidden_primitives
        )?;
    }
    for chunk in model.unknown_chunks() {
        writeln!(
            stdout,
            "skipped unknown chunk {} {} of the {} file",
            chunk.index,
            chunk.chunk_type.hex_id(),
            file_name(chunk.file)
        )?;
    }
    Ok(())
}

fn write_textures(
    out: &mut impl Write,
    summary: &GlbSummary,
    from_archive: bool,
) -> io::Result<()> {
    if !from_archive {
        return writeln!(out, "{NO_ARCHIVE}");
    }
    writeln!(
        out,
        "embedded {} images of {} pixels in {} of {} materials",
        summary.textures, summary.texture_pixels, summary.textured_materials, summary.materials
    )?;
    for fallback in &summary.texture_fallbacks {
        writeln!(
            out,
            "no {} texture {} for material {}: {}; the plain colour is used",
            fallback.role,
            printable(&fallback.file),
            printable(&fallback.material),
            printable(&fallback.reason)
        )?;
    }
    Ok(())
}

fn glb_json<'a>(
    source: &'a ModelSource,
    animations: &AnimationSet,
    out: &Path,
    written: &'a GlbGeometryPublication,
    no_archive: bool,
) -> Result<GlbJson<'a>> {
    let summary = &written.summary;
    let receipt = written
        .receipt
        .file_name()
        .context("missing GLB origin companion basename")?;
    Ok(GlbJson {
        model: source.name(),
        out: json::path(out),
        origin_receipt: json::path(Path::new(receipt)),
        nodes: summary.nodes,
        meshes: summary.meshes,
        primitives: summary.primitives,
        materials: summary.materials,
        textured_materials: summary.textured_materials,
        textures: summary.textures,
        texture_pixels: summary.texture_pixels,
        texture_fallbacks: summary
            .texture_fallbacks
            .iter()
            .filter(|_| !no_archive)
            .map(|fallback| FallbackJson {
                material: &fallback.material,
                role: fallback.role.name(),
                file: &fallback.file,
                reason: &fallback.reason,
            })
            .collect(),
        textures_note: no_archive.then_some(NO_ARCHIVE),
        default_material_ids: summary.default_material_ids,
        hidden_primitives: summary.hidden_primitives,
        bones: summary.bones,
        skinned_meshes: summary.skinned_meshes,
        vertices: summary.vertices,
        triangles: summary.triangles,
        bytes: summary.bytes,
        library: LibraryInfo::of(source.model()),
        animations: summary.animations.iter().map(WrittenJson::of).collect(),
        unread_animations: animations.unread.iter().map(UnreadInfo::of).collect(),
        unknown_chunks: source.model().unknown_chunks().map(ChunkInfo::of).collect(),
    })
}
