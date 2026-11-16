// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{FieldPath, MaterialInfo, ModelArchiveSource};

use crate::content_origin::EntryPageSources;
use crate::error::ToolError;
use crate::model_item_page::{self, EvidenceError, ModelItemPage};
use crate::model_shapes::{ModelItemOrigins, ModelItemsResult};

pub(crate) fn list(request: &ModelItemPage<'_>) -> Result<ModelItemsResult, ToolError> {
    let library = request.source.model().materials.library();
    let materials = library.map_or(&[][..], |library| library.materials.as_slice());
    let multi = library.is_some_and(|library| !library.single);
    let values = request
        .page
        .slice(materials)
        .iter()
        .enumerate()
        .map(|(index, material)| {
            let id = multi.then_some(request.page.offset() + index);
            serde_json::to_value(MaterialInfo::of(material, id)).map_err(ToolError::from)
        });
    request.publish(materials.len(), values, |range, cursor| {
        origins(request, range, cursor)
    })
}

fn origins(
    request: &ModelItemPage<'_>,
    rows: Range<usize>,
    cursor: Option<&str>,
) -> Result<ModelItemOrigins, EvidenceError> {
    let entry =
        EntryPageSources::from_origin(request.primary().and_then(ModelArchiveSource::origin))?;
    let root = FieldPath::parse("/structuredContent")?;
    let mut items = root.clone();
    items.push_field("items")?;
    let proof = request
        .source
        .material_page_origins(&entry.uses, &items, rows)
        .map_err(|error| EvidenceError::Material(Box::new(error)))?;
    let mut fields = proof.items;
    fields.insert(items, proof.count.clone())?;
    model_item_page::put(&mut fields, &root, "archive", entry.local())?;
    model_item_page::put(&mut fields, &root, "section", entry.local())?;
    model_item_page::put(&mut fields, &root, "model", entry.path)?;
    model_item_page::put(&mut fields, &root, "total", proof.count.clone())?;
    if cursor.is_some() {
        model_item_page::put(&mut fields, &root, "next_cursor", proof.count)?;
    }
    Ok(ModelItemOrigins {
        field_origins: fields,
        provenance: entry.uses.freeze()?,
    })
}
