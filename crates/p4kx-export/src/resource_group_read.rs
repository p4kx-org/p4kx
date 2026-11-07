// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use crate::contract::ResourceGroup;
use crate::guid_text::CatalogGuid;
use crate::record_fields;
use crate::source_fields::{FieldError, Fields};
use crate::texts::Texts;

/// A projected group and typed source-coverage diagnostics.
pub(crate) struct ResourceGroupRead<'source> {
    pub(crate) value: ResourceGroup<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Projects all group declarations while retaining the existing chosen parent.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    texts: &Texts<'source>,
    parent_guid: Option<CatalogGuid>,
) -> Result<ResourceGroupRead<'source>, FieldError> {
    let fields = Fields::new(tree.root(), "");
    let mut notes = Vec::new();
    fields.check(
        &[
            "displayName",
            "description",
            "defaultThumbnailPath",
            "defaultThumbnailPathSVG",
            "groups",
            "resources",
        ],
        &mut notes,
    );
    Ok(ResourceGroupRead {
        value: ResourceGroup {
            guid: CatalogGuid(tree.guid()),
            class_name: record_fields::class_name(tree),
            record_path: tree.record_path(),
            name: texts.localised(fields.locale("displayName")?),
            parent_guid,
            description: texts.localised(fields.locale("description")?),
            default_thumbnail_path: fields.text("defaultThumbnailPath")?,
            default_thumbnail_path_svg: fields.text("defaultThumbnailPathSVG")?,
            groups: fields.references("groups")?,
            resources: fields.references("resources")?,
        },
        notes,
    })
}
