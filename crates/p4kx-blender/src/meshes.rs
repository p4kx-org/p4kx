// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{BufWriter, Write};
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;

use p4kx_gltf::{GlbOptions, GlbSummary, GltfError, Scene, SceneNode, ScenePart, write_scene_glb};
use p4kx_model::{Material, Transform};
use rayon::prelude::*;

use crate::items::Item;
use crate::manifest::MESHES_FOLDER;
use crate::mesh_options::package_mesh_options;
use crate::package_receipt::PackageFile;
use crate::plan::{MeshPlan, mesh_id};
use crate::{PackageError, PackageResult, PackageWriteContext};

pub(crate) struct WrittenMesh {
    pub(crate) summary: GlbSummary,
    pub(crate) file: PackageFile,
}

/// Writes the GLB file `<folder>/<id>.glb` of every distinct mesh of `plan`, in parallel.
pub(crate) fn write_all(
    items: &[Item<'_>],
    plan: &MeshPlan,
    options: &GlbOptions,
    folder: &Path,
    cancelled: &PackageWriteContext<'_>,
    fallback: Option<&Material>,
) -> PackageResult<Vec<WrittenMesh>> {
    (0..plan.len())
        .into_par_iter()
        .map(|index| {
            cancelled.check()?;
            write_one(items, plan, index, options, folder, cancelled, fallback)
        })
        .collect()
}

fn write_one(
    items: &[Item<'_>],
    plan: &MeshPlan,
    index: usize,
    options: &GlbOptions,
    folder: &Path,
    context: &PackageWriteContext<'_>,
    fallback: Option<&Material>,
) -> PackageResult<WrittenMesh> {
    let id = mesh_id(index);
    let first = plan
        .first(index)
        .and_then(|position| items.get(position)?.drawn);
    let mut scene = Scene::new();
    let mut name = String::new();
    if let Some(first) = first {
        first.label.clone_into(&mut name);
        let drawn = ScenePart::new(first.model, first.materials);
        let drawn = fallback.map_or(drawn, |material| drawn.with_fallback(material));
        let drawn = first
            .palette
            .map_or(drawn, |palette| drawn.with_palette(palette));
        let node = SceneNode::new(first.label, None, Transform::IDENTITY).with_part(drawn);
        scene
            .push(node)
            .map_err(|source| mesh_error(&id, &name, source))?;
    }
    let filename = format!("{id}.glb");
    let path = folder.join(&filename);
    let mut glb = Vec::new();
    let summary = write_scene_glb(
        &scene,
        &package_mesh_options(options).with_material_indices(),
        &mut glb,
    )
    .map_err(|source| mesh_error(&id, &name, source))?;
    let (file, ownership) = context.create_file(&path)?.into_parts();
    let mut sink = BufWriter::new(file);
    let mut writer = OriginDigestWriter::new(&mut sink);
    writer
        .write_all(&glb)
        .map_err(|source| PackageError::write(&path, source))?;
    let digest = writer
        .finish()
        .map_err(|source| PackageError::write(&path, source))?;
    ownership.ensure_writable(&path, context.cancel_flag())?;
    sink.get_ref()
        .sync_all()
        .map_err(|source| PackageError::write(&path, source))?;
    Ok(WrittenMesh {
        summary,
        file: PackageFile::new(format!("{MESHES_FOLDER}/{filename}"), &digest),
    })
}

fn mesh_error(id: &str, name: &str, source: GltfError) -> PackageError {
    PackageError::Mesh {
        id: id.to_owned(),
        name: name.to_owned(),
        source,
    }
}
