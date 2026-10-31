// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use p4kx_gltf::{GlbOptions, GlbSummary, GltfError, Scene, SceneNode, ScenePart, write_scene_glb};
use p4kx_model::Transform;
use rayon::prelude::*;

use crate::items::Item;
use crate::mesh_options::package_mesh_options;
use crate::plan::{MeshPlan, mesh_id};
use crate::{Cancelled, PackageError, PackageResult};

/// Writes the GLB file `<folder>/<id>.glb` of every distinct mesh of `plan`, in parallel.
pub(crate) fn write_all(
    items: &[Item<'_>],
    plan: &MeshPlan,
    options: &GlbOptions,
    folder: &Path,
    cancelled: &Cancelled<'_>,
) -> PackageResult<Vec<GlbSummary>> {
    (0..plan.len())
        .into_par_iter()
        .map(|index| {
            cancelled.check()?;
            write_one(items, plan, index, options, folder)
        })
        .collect()
}

fn write_one(
    items: &[Item<'_>],
    plan: &MeshPlan,
    index: usize,
    options: &GlbOptions,
    folder: &Path,
) -> PackageResult<GlbSummary> {
    let id = mesh_id(index);
    let first = plan
        .first(index)
        .and_then(|position| items.get(position)?.drawn);
    let mut scene = Scene::new();
    let mut name = String::new();
    if let Some(first) = first {
        first.label.clone_into(&mut name);
        let drawn = ScenePart::new(first.model, first.materials);
        let drawn = first
            .palette
            .map_or(drawn, |palette| drawn.with_palette(palette));
        let node = SceneNode::new(first.label, None, Transform::IDENTITY).with_part(drawn);
        scene
            .push(node)
            .map_err(|source| mesh_error(&id, &name, source))?;
    }
    let path = folder.join(format!("{id}.glb"));
    let mut glb = Vec::new();
    let summary = write_scene_glb(
        &scene,
        &package_mesh_options(options).with_material_indices(),
        &mut glb,
    )
    .map_err(|source| mesh_error(&id, &name, source))?;
    let file = File::create_new(&path).map_err(|source| PackageError::write(&path, source))?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(&glb)
        .and_then(|()| writer.flush())
        .map_err(|source| PackageError::write(&path, source))?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|source| PackageError::write(&path, source))?;
    Ok(summary)
}

fn mesh_error(id: &str, name: &str, source: GltfError) -> PackageError {
    PackageError::Mesh {
        id: id.to_owned(),
        name: name.to_owned(),
        source,
    }
}
