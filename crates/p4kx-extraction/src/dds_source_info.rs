// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::{DdsHeader, DdsRawHeader, TextureError};

#[path = "dds_source_words.rs"]
mod words;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub use words::{DdsSourceWord, raw_dds_source_words};

use crate::DecimalU64;
use crate::dds_raw_info::{DdsDx10Info, DdsLegacyInfo};

/// Version of the additive DDS source-header metadata projection.
pub const DDS_SOURCE_INFO_VERSION: u8 = 1;

/// Complete authored header words and separate reader-normalized topology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct DdsSourceInfo {
    /// Metadata contract version, independent of any game or DDS version.
    #[schemars(extend("const" = DDS_SOURCE_INFO_VERSION))]
    pub schema_version: u8,
    /// Header Entry declared decoded length or disk file length, excluding separate siblings.
    #[serde(deserialize_with = "deserialize_header_source_bytes")]
    pub header_source_bytes: DecimalU64,
    /// Every declared source header word without defaults.
    pub raw: DdsRawHeaderInfo,
    /// Reader-derived topology and layout, without an image-decoding claim.
    pub normalized: DdsNormalizedInfo,
}

/// Authored source header, including an explicitly absent DX10 extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(transform = crate::projection_schema::require_properties)]
#[non_exhaustive]
pub struct DdsRawHeaderInfo {
    /// Whether the four DDS magic bytes preceded the header.
    pub magic_present: bool,
    /// Actual known-profile header length, including optional magic and DX10 extension.
    #[schemars(range(max = p4kx_texture::MAX_HEADER_LEN))]
    pub header_length: usize,
    /// All thirty-one authored legacy words.
    pub legacy: DdsLegacyInfo,
    /// All five extension words, or null when no DX10 extension was declared.
    pub dx10: Option<DdsDx10Info>,
}

/// Topology after applying the existing reader's defaults and validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(transform = crate::projection_schema::require_properties)]
#[non_exhaustive]
pub struct DdsNormalizedInfo {
    /// Existing reader format label; raw format declarations remain separate.
    pub format: String,
    /// Normalized largest-mip width.
    #[schemars(range(max = u32::MAX))]
    pub width: u32,
    /// Normalized largest-mip height.
    #[schemars(range(max = u32::MAX))]
    pub height: u32,
    /// Normalized largest-mip depth.
    #[schemars(range(max = u32::MAX))]
    pub depth: u32,
    /// Normalized mip count.
    #[schemars(range(max = u32::MAX))]
    pub mip_count: u32,
    /// Normalized array size.
    #[schemars(range(max = u32::MAX))]
    pub array_size: u32,
    /// Reader-normalized face count per array element.
    #[schemars(range(max = u32::MAX))]
    pub faces: u32,
    /// Existing reader cubemap predicate; raw legacy face bits remain in caps2.
    pub cubemap: bool,
    /// Whether the existing reader classifies this format as preserving high-range pixels.
    pub reader_high_range: bool,
    /// Existing encoded block layout; null does not prove a corrupt source.
    pub block_layout: Option<DdsBlockLayoutInfo>,
}

/// Encoded byte layout known to the current reader, independent of codec availability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct DdsBlockLayoutInfo {
    /// Block width in pixels.
    #[schemars(range(max = u32::MAX))]
    pub block_width: u32,
    /// Block height in pixels.
    #[schemars(range(max = u32::MAX))]
    pub block_height: u32,
    /// Encoded bytes per block.
    #[schemars(range(max = u32::MAX))]
    pub bytes_per_block: u32,
}

impl DdsSourceInfo {
    /// Projects one supplied header prefix without opening, decoding or merging any source.
    ///
    /// # Errors
    /// Returns the current DDS header's structural or normalized-topology error.
    pub fn parse(prefix: &[u8], header_source_bytes: u64) -> Result<Self, TextureError> {
        Self::from_raw(&DdsRawHeader::parse(prefix)?, header_source_bytes)
    }

    /// Projects an already captured header without reparsing its source bytes.
    ///
    /// # Errors
    /// Returns the current reader's invalid-topology error while raw fields stay available.
    pub fn from_raw(raw: &DdsRawHeader, header_source_bytes: u64) -> Result<Self, TextureError> {
        Self::from_raw_with_header(raw, header_source_bytes).map(|(info, _)| info)
    }

    /// Projects captured source fields together with their normalized reader header.
    ///
    /// # Errors
    /// Returns invalid-topology errors without modifying the captured source fields.
    pub fn from_raw_with_header(
        raw: &DdsRawHeader,
        header_source_bytes: u64,
    ) -> Result<(Self, DdsHeader), TextureError> {
        let normalized = DdsHeader::from_raw(raw)?;
        Ok((
            Self::of_parts(raw, &normalized, header_source_bytes),
            normalized,
        ))
    }

    pub(crate) fn of_parts(raw: &DdsRawHeader, normalized: &DdsHeader, size: u64) -> Self {
        Self {
            schema_version: DDS_SOURCE_INFO_VERSION,
            header_source_bytes: DecimalU64::new(size),
            raw: DdsRawHeaderInfo::of(raw),
            normalized: DdsNormalizedInfo::of(normalized),
        }
    }
}

impl DdsRawHeaderInfo {
    /// Projects structurally captured raw fields even when topology normalization is invalid.
    #[must_use]
    pub fn of(raw: &DdsRawHeader) -> Self {
        Self {
            magic_present: raw.magic_present(),
            header_length: raw.header_len(),
            legacy: DdsLegacyInfo::of(raw.legacy()),
            dx10: raw.dx10().map(DdsDx10Info::of),
        }
    }
}

impl DdsNormalizedInfo {
    fn of(header: &DdsHeader) -> Self {
        Self {
            format: header.format().to_string(),
            width: header.width(),
            height: header.height(),
            depth: header.depth(),
            mip_count: header.mip_count(),
            array_size: header.array_size(),
            faces: header.faces(),
            cubemap: header.is_cubemap(),
            reader_high_range: header.format().is_high_range(),
            block_layout: header.format().layout().map(|layout| DdsBlockLayoutInfo {
                block_width: layout.block_width,
                block_height: layout.block_height,
                bytes_per_block: layout.bytes_per_block,
            }),
        }
    }
}

fn deserialize_header_source_bytes<'de, D: serde::Deserializer<'de>>(
    source: D,
) -> Result<DecimalU64, D::Error> {
    use serde::de::Error as _;
    let text = String::deserialize(source)?;
    if text.is_empty()
        || text.len() > 20
        || text.len() > 1 && text.starts_with('0')
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(D::Error::custom(
            "invalid canonical unsigned DDS header length",
        ));
    }
    text.parse().map(DecimalU64::new).map_err(D::Error::custom)
}
