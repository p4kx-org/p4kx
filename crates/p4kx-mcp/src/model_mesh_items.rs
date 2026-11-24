// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model_info::write_submesh_origins;
use p4kx_extraction::{FieldOrigins, FieldPath, LodInfo, MeshInfo, ModelArchiveSource};
use serde::Serialize;

use crate::content_origin::EntryPageSources;
use crate::error::ToolError;
use crate::model_item_page::{self, EvidenceError, ModelItemPage};
use crate::model_shapes::{ModelItemOrigins, ModelItemsResult};

pub(crate) fn list(request: &ModelItemPage<'_>) -> Result<ModelItemsResult, ToolError> {
    let lods = LodInfo::of_model(request.source.model());
    let meshes: Vec<_> = lods
        .iter()
        .enumerate()
        .flat_map(|(lod_index, lod)| {
            lod.meshes
                .iter()
                .enumerate()
                .map(move |(mesh_index, mesh)| MeshRow {
                    lod_index,
                    mesh_index,
                    item: LodMesh {
                        lod: lod.level,
                        mesh,
                    },
                })
        })
        .collect();
    let values = request
        .page
        .slice(&meshes)
        .iter()
        .map(|row| serde_json::to_value(&row.item).map_err(ToolError::from));
    request.publish(meshes.len(), values, |range, cursor| {
        field_origins(request, &meshes, range, cursor)
    })
}

struct MeshRow<'a> {
    lod_index: usize,
    mesh_index: usize,
    item: LodMesh<'a>,
}

#[derive(Serialize)]
struct LodMesh<'a> {
    lod: u32,
    #[serde(flatten)]
    mesh: &'a MeshInfo,
}

fn field_origins(
    request: &ModelItemPage<'_>,
    meshes: &[MeshRow<'_>],
    range: Range<usize>,
    cursor: Option<&str>,
) -> Result<ModelItemOrigins, EvidenceError> {
    let entry =
        EntryPageSources::from_origin(request.primary().and_then(ModelArchiveSource::origin))?;
    let unknown = entry.uses.origin(&[], false);
    let mut fields = FieldOrigins::new();
    let root = FieldPath::parse("/structuredContent")?;
    for (field, origin) in [
        ("archive", entry.local()),
        ("model", entry.path.clone()),
        ("section", entry.local()),
        ("total", unknown.clone()),
        ("items", unknown.clone()),
    ] {
        model_item_page::put(&mut fields, &root, field, origin)?;
    }
    if cursor.is_some() {
        model_item_page::put(&mut fields, &root, "next_cursor", unknown.clone())?;
    }
    let selected = meshes.get(range).ok_or(EvidenceError::MissingRows)?;
    let mut path = root;
    path.push_field("items")?;
    for (local, row) in selected.iter().enumerate() {
        path.push_index(local)?;
        for field in [
            "lod",
            "node",
            "vertices",
            "triangles",
            "normals",
            "normals_computed",
            "tangents",
            "colours",
            "second_uvs",
            "non_finite_uvs",
            "skinned",
        ] {
            model_item_page::put(&mut fields, &path, field, unknown.clone())?;
        }
        write_submesh_origins(
            request.source,
            row.lod_index,
            row.mesh_index,
            &path,
            &entry.uses,
            &mut fields,
        )
        .map_err(EvidenceError::Mesh)?;
        path.pop();
    }
    Ok(ModelItemOrigins {
        field_origins: fields,
        provenance: entry.uses.freeze()?,
    })
}
