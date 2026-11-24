// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::ModelFile;
use p4kx_extraction::model_info::{ModelBoneOrigins, ModelNodeOrigins, ModelOverviewMeshOrigins};
use p4kx_extraction::{
    FieldOrigin, FieldOrigins, FieldPath, ModelArchiveSource, ModelSource, SourceUseRole,
    model_material_selector,
};
use serde_json::Value;

use crate::content_origin::EntryPageSources;
use crate::model_item_page::{self, EvidenceError};
use crate::model_shapes::ModelItemOrigins;

pub(super) fn capture(
    source: &ModelSource,
    library: &Value,
    material_ids: &[u16],
) -> Result<ModelItemOrigins, EvidenceError> {
    let model = source.model();
    let primary = source
        .archive_sources()
        .iter()
        .find(|read| read.file() == ModelFile::Model)
        .and_then(ModelArchiveSource::origin);
    let entry = EntryPageSources::from_origin(primary)?;
    let root = FieldPath::parse("/structuredContent")?;
    let unknown = entry.uses.origin(&[], false);
    let mut library_path = FieldPath::parse("/structuredContent/library")?;
    let mut fields = source
        .loaded_material_summary_origins(
            &entry.uses,
            &library_path,
            &FieldPath::parse("/structuredContent/counts/materials")?,
        )
        .map_err(|error| EvidenceError::Material(Box::new(error)))?;

    let geometry = if primary.is_some() && model.geometry.is_some() {
        entry.origin_when(&[entry.leaf_path()?], false)
    } else {
        unknown.clone()
    };
    let material = match (primary, model_material_selector(model)) {
        (Some(_), Some(location)) => {
            let word = entry.subresource(location, SourceUseRole::Conversion)?;
            entry.origin_when(&[word], false)
        }
        _ => unknown.clone(),
    };
    for (name, origin) in [
        ("archive", entry.local()),
        ("model", entry.path.clone()),
        ("geometry", geometry),
        ("material_library", material.clone()),
        ("library", unknown.clone()),
        ("counts", unknown.clone()),
        ("lods", unknown.clone()),
    ] {
        model_item_page::put(&mut fields, &root, name, origin)?;
    }
    if library.get("path").is_some() {
        model_item_page::put(&mut fields, &library_path, "path", material)?;
    }
    write_library_children(&mut fields, &mut library_path, library, &unknown)?;
    write_counts(source, &entry, &mut fields, &unknown)?;
    write_lod_shape(source, &mut fields, &unknown)?;
    ModelOverviewMeshOrigins::new(source, &entry.uses)
        .write(&root, material_ids, &mut fields)
        .map_err(EvidenceError::Mesh)?;
    Ok(ModelItemOrigins {
        field_origins: fields,
        provenance: entry.uses.freeze()?,
    })
}

fn write_counts(
    source: &ModelSource,
    entry: &EntryPageSources,
    fields: &mut FieldOrigins,
    unknown: &FieldOrigin,
) -> Result<(), EvidenceError> {
    let counts = FieldPath::parse("/structuredContent/counts")?;
    let nodes = ModelNodeOrigins::capture(source, &entry.uses).map_err(EvidenceError::Node)?;
    let bones = ModelBoneOrigins::capture(source, &entry.uses).map_err(EvidenceError::Bone)?;
    model_item_page::put(fields, &counts, "nodes", nodes.collection_origin())?;
    model_item_page::put(fields, &counts, "bones", bones.collection_origin())?;
    for name in [
        "materials",
        "lods",
        "meshes",
        "animations",
        "unread_animations",
        "chunks",
    ] {
        let mut field = counts.clone();
        field.push_field(name)?;
        if fields.get(&field).is_none() {
            fields.insert(field, unknown.clone())?;
        }
    }
    Ok(())
}

fn write_lod_shape(
    source: &ModelSource,
    fields: &mut FieldOrigins,
    unknown: &FieldOrigin,
) -> Result<(), EvidenceError> {
    let mut path = FieldPath::parse("/structuredContent/lods")?;
    for index in 0..source.model().lods.len() {
        path.push_index(index)?;
        fields.insert(path.clone(), unknown.clone())?;
        for name in ["level", "meshes"] {
            model_item_page::put(fields, &path, name, unknown.clone())?;
        }
        path.pop();
    }
    Ok(())
}

fn write_library_children(
    fields: &mut FieldOrigins,
    path: &mut FieldPath,
    value: &Value,
    unknown: &FieldOrigin,
) -> Result<(), EvidenceError> {
    if fields.get(path).is_none() {
        fields.insert(path.clone(), unknown.clone())?;
    }
    match value {
        Value::Object(members) => {
            for (name, child) in members {
                path.push_field(name)?;
                write_library_children(fields, path, child, unknown)?;
                path.pop();
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                path.push_index(index)?;
                write_library_children(fields, path, child, unknown)?;
                path.pop();
            }
        }
        _ => {}
    }
    Ok(())
}
