// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Entry;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::loose_archive_path::LooseArchivePath;
use crate::schema_fields::every_field_required;

/// Original Archive target paths of the geometry and mineable settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct GeologyReferences {
    /// The authored geometry path lookup.
    pub geometry: LooseArchivePath,
    /// The authored material path lookup.
    pub material: LooseArchivePath,
    /// The authored mineable configuration geometry lookup.
    pub mineable_config: LooseArchivePath,
    /// The authored mineable material lookup.
    pub mineable_material: LooseArchivePath,
}

pub(super) fn geometry(entry: &Entry) -> bool {
    has_extension(entry, "cgf")
}

pub(super) fn material(entry: &Entry) -> bool {
    has_extension(entry, "mtl")
}

fn has_extension(entry: &Entry, expected: &str) -> bool {
    entry
        .path()
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case(expected))
}
