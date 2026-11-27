// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model_info::ModelBoneOrigins;
use p4kx_extraction::{BoneInfo, FieldOrigins, FieldPath, ModelArchiveSource};

use crate::content_origin::EntryPageSources;
use crate::error::ToolError;
use crate::model_item_page::{self, EvidenceError, ModelItemPage};
use crate::model_shapes::{ModelItemOrigins, ModelItemsResult};

pub(crate) fn list(request: &ModelItemPage<'_>) -> Result<ModelItemsResult, ToolError> {
    let bones = &request.source.model().bones;
    let values = request
        .page
        .slice(bones)
        .iter()
        .enumerate()
        .map(|(position, bone)| {
            serde_json::to_value(BoneInfo::of_bone(request.page.offset() + position, bone))
                .map_err(ToolError::from)
        });
    request.publish(bones.len(), values, |range, cursor| {
        field_origins(request, range, cursor)
    })
}

fn field_origins(
    request: &ModelItemPage<'_>,
    range: Range<usize>,
    cursor: Option<&str>,
) -> Result<ModelItemOrigins, EvidenceError> {
    let entry =
        EntryPageSources::from_origin(request.primary().and_then(ModelArchiveSource::origin))?;
    let proof =
        ModelBoneOrigins::capture(request.source, &entry.uses).map_err(EvidenceError::Bone)?;
    let mut fields = FieldOrigins::new();
    let root = FieldPath::parse("/structuredContent")?;
    for (field, origin) in [
        ("archive", entry.local()),
        ("model", entry.path.clone()),
        ("section", entry.local()),
    ] {
        model_item_page::put(&mut fields, &root, field, origin)?;
    }
    let selection = proof.collection_origin();
    for field in ["total", "items"] {
        model_item_page::put(&mut fields, &root, field, selection.clone())?;
    }
    if cursor.is_some() {
        model_item_page::put(&mut fields, &root, "next_cursor", selection)?;
    }
    let mut path = root;
    path.push_field("items")?;
    for (position, ordinal) in range.enumerate() {
        path.push_index(position)?;
        proof
            .write_bone(&mut fields, &path, ordinal)
            .map_err(EvidenceError::Bone)?;
        path.pop();
    }
    Ok(ModelItemOrigins {
        field_origins: fields,
        provenance: entry.uses.freeze()?,
    })
}
