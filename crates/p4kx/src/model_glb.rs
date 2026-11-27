// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use p4kx_assembly::animation_choice;
use p4kx_extraction::model::{LodSelection, Model};
use p4kx_extraction::model_info::{file_name, write_chunk_origins, write_glb_animation_origins};
use p4kx_extraction::{
    AnimationChoice, AnimationSet, ArchiveAnimationRead, ArchiveTextures, ChunkInfo, FieldOrigins,
    FieldPath, GlbGeometryPublication, GlbOptions, GlbSummary, LibraryInfo, MaterialMode,
    ModelSource, PathIndex, SourceInventory, SourceUses, UnreadInfo,
};
use serde::Serialize;

use crate::cli_model::{MaterialsArg, ModelAction, ModelFileSource};
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
    #[serde(flatten)]
    origins: GlbReportOrigins,
}

#[derive(Serialize)]
struct GlbReportOrigins {
    unknown_chunks: Vec<ChunkInfo>,
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

impl GlbReportOrigins {
    fn capture(
        source: &ModelSource,
        animations: &AnimationSet,
        summary: &GlbSummary,
        observed: Option<&ArchiveAnimationRead>,
    ) -> Result<Self> {
        let mut unknown_chunks = Vec::new();
        let uses = SourceUses::with_compact_use_refs();
        let mut field_origins = source.model_and_library_report_origins(
            &uses,
            &FieldPath::parse("/model")?,
            &FieldPath::parse("/library")?,
        )?;
        let mut path = FieldPath::parse("/unknown_chunks")?;
        for (row, (position, chunk)) in source.model().unknown_chunks_with_positions().enumerate() {
            path.push_index(row)?;
            write_chunk_origins(source, position, &path, &uses, &mut field_origins)?;
            unknown_chunks.push(ChunkInfo::of(chunk));
            path.pop();
        }
        write_glb_animation_origins(
            observed,
            animations,
            &summary.animations,
            &uses,
            &mut field_origins,
        )?;
        Ok(Self {
            unknown_chunks,
            field_origins,
            provenance: uses.freeze()?,
        })
    }
}

#[derive(Clone, Copy)]
enum ReportMode<'a> {
    Text,
    Json(Option<&'a ArchiveAnimationRead>),
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
        export_archive(source, entry, &options, out, choice.as_ref(), as_json)
    } else {
        let model = ModelSource::from_file_with(&source.path, lods)?;
        let found = disk_animations(source, choice.as_ref())?;
        let written = model.write_glb_with_geometry_receipt(&found, &options, out, None)?;
        let mode = if as_json {
            ReportMode::Json(None)
        } else {
            ReportMode::Text
        };
        report(&model, &found, &options, &written, out, false, mode)
    }
}

fn export_archive(
    source: &ModelFileSource,
    entry: &str,
    options: &GlbOptions,
    out: &Path,
    choice: Option<&AnimationChoice>,
    as_json: bool,
) -> Result<()> {
    let lods = LodSelection::Only(options.lod);
    let archive = crate::key::open(&source.path)?;
    let index = PathIndex::new(&archive);
    let model = if as_json {
        ModelSource::from_index_with_drawn_material_summary_origins(&index, entry, lods)?
    } else {
        ModelSource::from_index_with(&index, entry, lods)?
    };
    let observed = if as_json {
        choice
            .map(|choice| {
                ArchiveAnimationRead::for_model(&index, entry, choice, &source.animation_sources)
            })
            .transpose()?
    } else {
        None
    };
    let ordinary;
    let found = if let Some(observed) = &observed {
        observed.set()
    } else {
        ordinary = archive_animations(&index, entry, source, choice)?;
        &ordinary
    };
    let textures = ArchiveTextures::new(&index);
    let written = model.write_glb_with_geometry_receipt(found, options, out, Some(&textures))?;
    let mode = if as_json {
        ReportMode::Json(observed.as_ref())
    } else {
        ReportMode::Text
    };
    report(&model, found, options, &written, out, true, mode)
}

fn report(
    source: &ModelSource,
    animations: &AnimationSet,
    options: &GlbOptions,
    written: &GlbGeometryPublication,
    out: &Path,
    from_archive: bool,
    mode: ReportMode<'_>,
) -> Result<()> {
    let model = source.model();
    let summary = &written.summary;
    if let ReportMode::Json(observed) = mode {
        let no_archive = options.materials == MaterialMode::Textures && !from_archive;
        json::print(&glb_json(
            source, animations, out, written, no_archive, observed,
        )?)?;
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
    observed: Option<&ArchiveAnimationRead>,
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
        origins: GlbReportOrigins::capture(source, animations, summary, observed)?,
    })
}
