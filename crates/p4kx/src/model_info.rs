// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};

use anyhow::Result;
use p4kx_extraction::model::{ChunkUsage, MaterialStatus, Mesh, Model, Node};
use p4kx_extraction::model_info::{file_name, node_name, usage_name};
use p4kx_extraction::{AnimationInfo, AnimationListing, ModelInfo, ModelSource};

use crate::json;
use crate::model_animations::{count_line, write_table, write_unread};
use crate::model_bones::write_bones;
use crate::model_text::printable;

const NONE: &str = "-";

/// Prints the text or JSON of `p4kx model info`.
pub(crate) fn info(
    (source, listing): &(ModelSource, AnimationListing),
    as_json: bool,
) -> Result<()> {
    if as_json {
        json::print(&ModelInfo::of_listing(source, listing))?;
        return Ok(());
    }
    let model = source.model();
    let animations = AnimationInfo::of_listing(model, listing);
    let mut out = BufWriter::new(io::stdout().lock());
    writeln!(out, "Model:       {}", printable(source.name()))?;
    writeln!(
        out,
        "Geometry:    {}",
        printable(model.geometry.as_deref().unwrap_or(NONE))
    )?;
    writeln!(out, "Nodes:       {}", model.nodes.len())?;
    writeln!(out, "LODs:        {}", model.lods.len())?;
    writeln!(out, "Materials:   {}", materials(model))?;
    writeln!(out, "Bones:       {}", model.bones.len())?;
    writeln!(out, "Animations:  {}", count_line(listing))?;
    writeln!(out, "Chunks:      {}", chunk_counts(model))?;
    for issue in &model.issues {
        writeln!(out, "Issue:       {}", printable(&issue.to_string()))?;
    }
    write_lods(&mut out, model)?;
    write_materials(&mut out, model)?;
    write_nodes(&mut out, model)?;
    write_bones(&mut out, model)?;
    write_table(&mut out, &animations)?;
    write_unread(&mut out, &listing.unread)?;
    write_chunks(&mut out, model)?;
    out.flush()?;
    Ok(())
}

fn bone_count(bones: Option<&[p4kx_extraction::model::Bone]>) -> String {
    match bones.map(<[_]>::len) {
        None => NONE.to_owned(),
        Some(1) => "1 bone".to_owned(),
        Some(count) => format!("{count} bones"),
    }
}

fn write_lods(out: &mut impl Write, model: &Model) -> io::Result<()> {
    writeln!(out)?;
    writeln!(
        out,
        "{:>4}  {:>10}  {:>10}  {:>6}  {:>8}",
        "LOD", "Vertices", "Triangles", "Meshes", "Skeleton"
    )?;
    for lod in &model.lods {
        let skeleton = bone_count(lod.bones.as_deref());
        writeln!(
            out,
            "{:>4}  {:>10}  {:>10}  {:>6}  {skeleton:>8}",
            lod.level,
            lod.vertex_count(),
            lod.triangle_count(),
            lod.meshes.len()
        )?;
    }
    writeln!(out)?;
    writeln!(
        out,
        "{:>4}  {:<24}  {:>10}  {:>10}  {:<7}  {:<8}  {:<7}  {:<7}  {:>7}  Material IDs",
        "LOD",
        "Node",
        "Vertices",
        "Triangles",
        "Normals",
        "Tangents",
        "Colours",
        "Skinned",
        "Bad UVs"
    )?;
    for lod in &model.lods {
        for mesh in &lod.meshes {
            let ids: Vec<String> = mesh
                .submeshes
                .iter()
                .map(|submesh| submesh.material_id.to_string())
                .collect();
            writeln!(
                out,
                "{:>4}  {:<24}  {:>10}  {:>10}  {:<7}  {:<8}  {:<7}  {:<7}  {:>7}  {}",
                lod.level,
                printable(node_name(model, mesh.node)),
                mesh.positions.len(),
                mesh.indices.len() / 3,
                normals(mesh),
                yes_no(mesh.tangents.is_some()),
                yes_no(mesh.colours.is_some()),
                yes_no(mesh.influences.is_some()),
                mesh.non_finite_uvs,
                ids.join(", ")
            )?;
        }
    }
    Ok(())
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

const fn normals(mesh: &Mesh) -> &'static str {
    if mesh.normals_computed {
        "computed"
    } else {
        yes_no(mesh.normals.is_some())
    }
}

fn write_nodes(out: &mut impl Write, model: &Model) -> io::Result<()> {
    writeln!(out)?;
    writeln!(
        out,
        "{:>5}  {:<6}  {:<8}  {:<20}  Name",
        "Node", "Parent", "Type", "Scale"
    )?;
    for (index, node) in model.nodes.iter().enumerate() {
        let parent = node
            .parent
            .map_or_else(|| NONE.to_owned(), |parent| parent.to_string());
        let [x, y, z] = node.scale;
        let properties: Vec<String> = node
            .properties
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        let properties = if properties.is_empty() {
            String::new()
        } else {
            format!("  [{}]", printable(&properties.join(", ")))
        };
        writeln!(
            out,
            "{index:>5}  {parent:<6}  {:<8}  {:<20}  {}{properties}",
            node_type(node),
            format!("{x} {y} {z}"),
            printable(node_name(model, index))
        )?;
    }
    if !model.material_indices.is_empty() {
        let indices: Vec<String> = model.material_indices.iter().map(u16::to_string).collect();
        writeln!(out, "Material indices: {}", indices.join(", "))?;
    }
    Ok(())
}

fn node_type(node: &Node) -> String {
    match node.geometry_type {
        None => NONE.to_owned(),
        Some(0) => "geometry".to_owned(),
        Some(_) if node.is_helper() => "helper".to_owned(),
        Some(other) => format!("type {other}"),
    }
}

fn write_chunks(out: &mut impl Write, model: &Model) -> io::Result<()> {
    writeln!(out)?;
    writeln!(
        out,
        "{:<13}  {:>5}  {:<24}  {:>10}  Usage",
        "File", "Index", "Type", "Type ID"
    )?;
    for chunk in &model.chunks {
        writeln!(
            out,
            "{:<13}  {:>5}  {:<24}  {:>10}  {}",
            file_name(chunk.file),
            chunk.index,
            chunk.chunk_type.name().unwrap_or("unknown"),
            chunk.chunk_type.hex_id(),
            usage_name(chunk.usage)
        )?;
    }
    Ok(())
}
fn materials(model: &Model) -> String {
    let ids: Vec<String> = model.material_ids().iter().map(u16::to_string).collect();
    let ids = if ids.is_empty() {
        "no material IDs".to_owned()
    } else {
        format!("IDs {}", ids.join(", "))
    };
    model.material_library.as_ref().map_or_else(
        || format!("no material library, {ids}"),
        |library| {
            let target = model
                .materials
                .target()
                .map_or_else(String::new, |target| format!(" -> {}", printable(target)));
            format!("{}{target} ({}), {ids}", printable(library), status(model))
        },
    )
}

fn status(model: &Model) -> String {
    match &model.materials {
        MaterialStatus::Loaded(library) if library.single => "one material".to_owned(),
        MaterialStatus::Loaded(library) => format!("{} sub-materials", library.materials.len()),
        MaterialStatus::Missing { .. } => "missing".to_owned(),
        MaterialStatus::Unreadable { reason, .. } => format!("unreadable: {}", printable(reason)),
        _ => "not read".to_owned(),
    }
}

fn write_materials(out: &mut impl Write, model: &Model) -> io::Result<()> {
    let Some(library) = model.materials.library() else {
        return Ok(());
    };
    writeln!(out)?;
    writeln!(
        out,
        "{:>3}  {:<24}  {:<16}  {:<18}  {:<20}  {:>7}  Textures",
        "ID", "Material", "Shader", "Surface", "Diffuse", "Opacity"
    )?;
    for (position, material) in library.materials.iter().enumerate() {
        let id = if library.single {
            NONE.to_owned()
        } else {
            position.to_string()
        };
        let [red, green, blue] = material.diffuse;
        writeln!(
            out,
            "{id:>3}  {:<24}  {:<16}  {:<18}  {:<20}  {:>7}  {}",
            printable(or_none(&material.name)),
            printable(or_none(&material.shader)),
            printable(or_none(&material.surface_type)),
            format!("{red:.3} {green:.3} {blue:.3}"),
            material.opacity,
            material.textures.len()
        )?;
    }
    Ok(())
}

const fn or_none(text: &str) -> &str {
    if text.is_empty() { NONE } else { text }
}

fn chunk_counts(model: &Model) -> String {
    let count = |usage| {
        model
            .chunks
            .iter()
            .filter(|chunk| chunk.usage == usage)
            .count()
    };
    format!(
        "{} ({} read, {} skipped, {} unknown)",
        model.chunks.len(),
        count(ChunkUsage::Read),
        count(ChunkUsage::Skipped),
        count(ChunkUsage::Unknown)
    )
}
