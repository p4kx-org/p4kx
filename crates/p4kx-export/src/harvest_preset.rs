// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::guid_text::CatalogGuid;
use crate::record_fields::{reference, structs};

/// Returns the GUIDs of the Tag Records selected by the original geometry rules.
pub(crate) fn geometry_tags(parent: Option<&StructValue<'_>>) -> Vec<CatalogGuid> {
    structs(parent, "geometries")
        .filter_map(|geometry| reference(Some(geometry), "tag"))
        .collect()
}
