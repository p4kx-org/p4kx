// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::texture_preview::{DdnaMipReason, ObservedDdnaDds};
use p4kx_extraction::{
    FieldOrigin, OriginArtifactDigest, OriginDigestWriter, SourceInventory, SourceUses,
};
use p4kx_texture::{DdsTransfer, DxgiFormat, LocatedMip, TextureFormat as DdsFormat};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Write as _};

use crate::texture_source_metadata::metadata_error;
use crate::{DdsTextureAsset, DdsTextureFallback, PackageResult};

/// Consumer pairing and provenance for a split normal and smoothness DDS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct DdnaTexturePair {
    /// Texture ID of the independently written linear smoothness DDS.
    #[schemars(regex(pattern = "^t[0-9]{4,}$"))]
    pub smoothness: String,
    /// Convention of the normal samples stored in the primary DDS.
    pub normal_axis: DdnaNormalAxis,
    /// Stored sample that drives smoothness.
    pub channel: DdnaSmoothnessChannel,
    /// Level requested by the package caller.
    pub requested_mip: u32,
    /// Common original level retained in the two files.
    pub selected_mip: u32,
    /// Which input determined the common level.
    pub selection: DdnaSelection,
    /// Source inputs contributing to the common mip and associated assets.
    #[serde(deserialize_with = "read_origin")]
    pub source_origin: FieldOrigin,
    /// Original entries referenced by the pairing.
    #[schemars(with = "SourceInventory")]
    pub provenance: Value,
}

/// Normal map convention stored by the DDS primary image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DdnaNormalAxis {
    /// Original CryEngine green channel, inverted by the importer.
    CryengineYDown,
}

/// Channel that the separate texture stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DdnaSmoothnessChannel {
    /// Red samples of the positive-unsigned BC4 block.
    Red,
}

/// Reason for the common DDNA level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DdnaSelection {
    /// Both input files contain the requested mip.
    Requested,
    /// The normal DDS bounds the selected mip.
    PrimaryChain,
    /// The smoothness DDS bounds the selected mip.
    SmoothnessChain,
    /// Both input chains stop at the selected mip.
    BothChains,
}

fn read_origin<'de, D: serde::Deserializer<'de>>(input: D) -> Result<FieldOrigin, D::Error> {
    use p4kx_extraction::{FieldOriginSeed, OriginReaderLimits};
    use serde::de::DeserializeSeed as _;

    FieldOriginSeed::new(OriginReaderLimits {
        fields: 1,
        metadata_bytes: 16 * 1024,
        path_bytes: 256,
        field_uses: 256,
        reference_bytes: 64,
    })
    .deserialize(input)
}

impl DdnaTexturePair {
    pub(crate) fn bind(observed: &ObservedDdnaDds<'_>, smoothness: String) -> PackageResult<Self> {
        let uses = SourceUses::with_compact_use_refs();
        let source_origin = observed.pairing_origin(&uses).map_err(metadata_error)?;
        let provenance =
            serde_json::to_value(uses.freeze().map_err(metadata_error)?).map_err(metadata_error)?;
        let selection = match observed.reason {
            DdnaMipReason::Requested => DdnaSelection::Requested,
            DdnaMipReason::PrimaryChain => DdnaSelection::PrimaryChain,
            DdnaMipReason::SmoothnessChain => DdnaSelection::SmoothnessChain,
            DdnaMipReason::BothChains => DdnaSelection::BothChains,
            _ => return Err(metadata_error(io::Error::other("unknown DDNA selection"))),
        };
        Ok(Self {
            smoothness,
            normal_axis: DdnaNormalAxis::CryengineYDown,
            channel: DdnaSmoothnessChannel::Red,
            requested_mip: observed.requested_mip,
            selected_mip: observed.normal.read.mip.level(),
            selection,
            source_origin,
            provenance,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SmoothnessWritten {
    pub(crate) id: String,
    pub(crate) source: String,
    pub(crate) asset: DdsTextureAsset,
    pub(crate) pairing: DdnaTexturePair,
}

pub(crate) fn unsupported(read: &LocatedMip) -> Option<DdsTextureFallback> {
    const TEXTURE_2D: u32 = 3;
    let transfer = read.raw_header.transfer();
    let encoded = read.mip.header().format() == DdsFormat::Dxgi(DxgiFormat::BC3_UNORM)
        && read.raw_header.dx10().map_or_else(
            || read.raw_header.legacy().pixel_format.four_cc == *b"DXT5",
            |ext| ext.resource_dimension == TEXTURE_2D,
        );
    (!encoded || transfer.conflict || transfer.effective == DdsTransfer::Srgb)
        .then_some(DdsTextureFallback::NormalCoordinateTransform)
}

pub(crate) fn payload_digest(read: &LocatedMip) -> PackageResult<OriginArtifactDigest> {
    let mut writer = OriginDigestWriter::new(io::sink());
    writer.write_all(read.mip.data()).map_err(metadata_error)?;
    writer.finish().map_err(metadata_error)
}
