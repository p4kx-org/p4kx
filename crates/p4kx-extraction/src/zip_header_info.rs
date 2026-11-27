// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::ZipHeaderMetadata;
use schemars::JsonSchema;
use serde::Serialize;

/// Lossless fixed ZIP directory words without interpreting their flags or host system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct ZipHeaderInfo {
    /// Complete version-made-by word, including the host-system byte.
    pub raw_version_made_by: u16,
    /// Complete version-needed word.
    pub raw_version_needed: u16,
    /// All source general-purpose flag bits.
    pub general_purpose_flags: u16,
    /// Original disk-start word, including a possible ZIP64 marker.
    pub disk_start: u16,
    /// All internal-attribute bits.
    pub internal_attributes: u16,
    /// All external-attribute bits without host-specific decoding.
    #[schemars(range(min = 0, max = 4_294_967_295_u64))]
    pub external_attributes: u32,
}

impl ZipHeaderInfo {
    pub(crate) const fn of(header: &ZipHeaderMetadata) -> Self {
        Self {
            raw_version_made_by: header.raw_version_made_by(),
            raw_version_needed: header.raw_version_needed(),
            general_purpose_flags: header.general_purpose_flags(),
            disk_start: header.disk_start(),
            internal_attributes: header.internal_attributes(),
            external_attributes: header.external_attributes(),
        }
    }
}
