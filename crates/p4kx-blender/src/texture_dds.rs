// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::texture_preview::{ModelDdsRefusal, ObservedModelDds};
use p4kx_extraction::{
    Checksum, DdsSourceInfo, FieldOrigins, OriginArtifactDigest, PreviewPngEvidence,
    SourceInventory, SourceUseRole, SourceUses,
};
use p4kx_texture::{
    DdsAlphaAssociation, DdsAlphaCoverage, DxgiFormat, LocatedMip, TextureError, TextureFormat,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::PackageResult;
use crate::texture_encoding::TextureMode;
use crate::texture_field_origins;
use crate::texture_source_metadata::{metadata_error, read_field_origins};

/// Texture container available to the selected package consumer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PackageTextureContainer {
    /// RGBA PNG images.
    #[default]
    Png,
    /// Original mip bytes in formats accepted by the Blender package consumer.
    BlenderDds,
}

/// Why a requested DDS asset could not be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DdsTextureFallback {
    /// The package caller does not provide a physical DDS source.
    UnavailableSource,
    /// The required physical member does not exist.
    MissingMember,
    /// More than one physical member has the required path.
    AmbiguousMember,
    /// The original path could not be established.
    UnprovedMember,
    /// The output requires combining colour with a separate alpha or smoothness member.
    SplitAlpha,
    /// The consumer material slot requires one two-dimensional surface.
    UnsupportedTopology,
    /// The consumer has no verified sample-compatible decoder for this encoding and role.
    UnsupportedFormat,
    /// Blender does not decode the signed BC5 format.
    SignedBc5,
    /// The consumer would interpret associated RGB as straight opacity.
    PremultipliedAlpha,
    /// The header has undefined alpha-mode bits.
    ReservedAlphaMode,
    /// The material role requires a flipped normal Y channel.
    NormalCoordinateTransform,
    /// An input could not be read, parsed or verified completely.
    InvalidInput,
}

impl DdsTextureFallback {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::UnavailableSource => "dds_unavailable_source",
            Self::MissingMember => "dds_missing_member",
            Self::AmbiguousMember => "dds_ambiguous_member",
            Self::UnprovedMember => "dds_unproved_member",
            Self::SplitAlpha => "dds_split_alpha",
            Self::UnsupportedTopology => "dds_unsupported_topology",
            Self::UnsupportedFormat => "dds_unsupported_format",
            Self::SignedBc5 => "dds_signed_bc5",
            Self::PremultipliedAlpha => "dds_premultiplied_alpha",
            Self::ReservedAlphaMode => "dds_reserved_alpha_mode",
            Self::NormalCoordinateTransform => "dds_normal_coordinate_transform",
            Self::InvalidInput => "dds_invalid_input",
        }
    }

    pub(crate) const fn source_error(error: &ModelDdsRefusal) -> SourceFallback {
        use SourceFallback::{Png, Refused};
        match error {
            ModelDdsRefusal::Missing => Refused(Self::MissingMember),
            ModelDdsRefusal::Ambiguous => Refused(Self::AmbiguousMember),
            ModelDdsRefusal::UnsupportedRawPath => Refused(Self::UnprovedMember),
            ModelDdsRefusal::SplitAlpha => Png(Self::SplitAlpha),
            ModelDdsRefusal::Topology => Png(Self::UnsupportedTopology),
            ModelDdsRefusal::Dds(TextureError::UnsupportedFormat(_)) => {
                Png(Self::UnsupportedFormat)
            }
            _ => Refused(Self::InvalidInput),
        }
    }
}

pub(crate) enum SourceFallback {
    Png(DdsTextureFallback),
    Refused(DdsTextureFallback),
}

/// The archive directory's checksum of a completely verified original input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "hex", rename_all = "lowercase")]
#[non_exhaustive]
pub enum DdsTextureSourceChecksum {
    /// CRC-32 of decoded Entry bytes.
    Crc32(#[schemars(regex(pattern = "^[0-9a-f]{8}$"))] String),
    /// CRC-32C of decoded Entry bytes.
    Crc32c(#[schemars(regex(pattern = "^[0-9a-f]{8}$"))] String),
    /// SHA-256 of stored Entry bytes.
    Sha256(#[schemars(regex(pattern = "^[0-9a-f]{64}$"))] String),
}

impl DdsTextureSourceChecksum {
    fn of(checksum: Checksum) -> PackageResult<Self> {
        match checksum {
            Checksum::Crc32(_) => Ok(Self::Crc32(checksum.hex())),
            Checksum::Crc32c(_) => Ok(Self::Crc32c(checksum.hex())),
            Checksum::Sha256(_) => Ok(Self::Sha256(checksum.hex())),
            _ => Err(metadata_error(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "unknown archive checksum",
            ))),
        }
    }
}

/// A normalized one-surface DDS asset and its original selected mip evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct DdsTextureAsset {
    /// Byte length and SHA-256 of the DDS file, including its normalized header.
    pub artifact: OriginArtifactDigest,
    /// Raw and reader-normalized original header.
    pub dds: DdsSourceInfo,
    /// Original zero-based mip copied into this two-dimensional asset.
    pub mip_level: u32,
    /// Measured alpha samples of the original selected BC1, BC2 or BC3 image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_coverage: Option<DdsAlphaCoverage>,
    /// Original archive checksum of the header Entry.
    pub header_checksum: DdsTextureSourceChecksum,
    /// Original archive checksum of the split mip Entry when separate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_checksum: Option<DdsTextureSourceChecksum>,
    /// Source origins relative to this object; /artifact includes the actual copied bytes.
    #[serde(deserialize_with = "read_field_origins")]
    pub field_origins: FieldOrigins,
    /// Read and directory inputs referenced by the immediate origins.
    #[schemars(with = "SourceInventory")]
    pub provenance: Value,
}

impl DdsTextureAsset {
    pub(crate) fn bind(
        observed: ObservedModelDds<'_>,
        artifact: OriginArtifactDigest,
        alpha_coverage: Option<DdsAlphaCoverage>,
    ) -> PackageResult<Self> {
        let uses = SourceUses::with_compact_use_refs();
        let evidence = observed.evidence(&uses).map_err(metadata_error)?;
        let (header_checksum, part_checksum) =
            observed.checksum_uses(&uses).map_err(metadata_error)?;
        let mut fields =
            texture_field_origins::capture(&observed.metadata, &evidence, false, &uses)?;
        coverage_origins(&mut fields, &evidence, &uses, alpha_coverage)?;
        let mut artifact_uses = Vec::new();
        evidence
            .append_pixel_uses(&mut artifact_uses)
            .map_err(metadata_error)?;
        artifact_uses.push(
            evidence
                .color
                .header()
                .byte_use(
                    0,
                    observed.metadata.raw.header_length as u64,
                    "originalHeader",
                    SourceUseRole::Conversion,
                )
                .map_err(metadata_error)?,
        );
        let complete = evidence.is_complete();
        texture_field_origins::insert(
            &mut fields,
            "/artifact",
            uses.origin(&artifact_uses, complete),
        )?;
        texture_field_origins::insert(
            &mut fields,
            "/header_checksum",
            uses.origin(&[header_checksum], complete),
        )?;
        if let Some(part) = part_checksum {
            texture_field_origins::insert(
                &mut fields,
                "/part_checksum",
                uses.origin(&[part], complete),
            )?;
        }
        let provenance =
            serde_json::to_value(uses.freeze().map_err(metadata_error)?).map_err(metadata_error)?;
        Ok(Self {
            artifact,
            mip_level: observed.read.mip.level(),
            alpha_coverage,
            header_checksum: DdsTextureSourceChecksum::of(observed.header_checksum)?,
            part_checksum: observed
                .part_checksum
                .map(DdsTextureSourceChecksum::of)
                .transpose()?,
            dds: observed.metadata,
            field_origins: fields,
            provenance,
        })
    }
}

fn coverage_origins(
    fields: &mut FieldOrigins,
    evidence: &PreviewPngEvidence,
    uses: &SourceUses,
    coverage: Option<DdsAlphaCoverage>,
) -> PackageResult<()> {
    if coverage.is_some() {
        let mut contributors = Vec::new();
        evidence
            .append_pixel_uses(&mut contributors)
            .map_err(metadata_error)?;
        let origin = uses.origin(&contributors, evidence.is_complete());
        for path in [
            "/alpha_coverage",
            "/alpha_coverage/zero",
            "/alpha_coverage/partial",
            "/alpha_coverage/full",
        ] {
            texture_field_origins::insert(fields, path, origin.clone())?;
        }
    }
    Ok(())
}

pub(crate) fn unsupported(read: &LocatedMip, mode: TextureMode) -> Option<DdsTextureFallback> {
    use DdsTextureFallback::{NormalCoordinateTransform, SignedBc5, UnsupportedFormat};

    const D3D10_RESOURCE_DIMENSION_TEXTURE2D: u32 = 3;
    let TextureFormat::Dxgi(format) = read.mip.header().format() else {
        return Some(UnsupportedFormat);
    };
    match read.raw_header.alpha_association() {
        DdsAlphaAssociation::Premultiplied if mode.uses_colour_samples() => {
            return Some(DdsTextureFallback::PremultipliedAlpha);
        }
        DdsAlphaAssociation::Reserved => return Some(DdsTextureFallback::ReservedAlphaMode),
        _ => {}
    }
    if format == DxgiFormat::BC5_SNORM {
        return Some(SignedBc5);
    }
    if matches!(mode, TextureMode::Normal { .. }) {
        return Some(NormalCoordinateTransform);
    }
    let supported = read.raw_header.dx10().map_or_else(
        || {
            matches!(
                &read.raw_header.legacy().pixel_format.four_cc,
                b"DXT1" | b"DXT5" | b"ATI1" | b"ATI2" | b"BC5U"
            ) || matches!(format.0, 28 | 87)
        },
        |dx10| {
            dx10.resource_dimension == D3D10_RESOURCE_DIMENSION_TEXTURE2D
                && matches!(format.0, 28 | 29 | 71 | 72 | 77 | 78 | 83)
        },
    );
    let data_sample_compatible = format != DxgiFormat::BC5_UNORM || mode == TextureMode::Data;
    (!supported || !data_sample_compatible).then_some(UnsupportedFormat)
}

pub(crate) fn alpha_has_samples(
    observed: &ObservedModelDds<'_>,
    coverage: Option<DdsAlphaCoverage>,
) -> bool {
    let mip = &observed.read.mip;
    if matches!(
        mip.header().format(),
        TextureFormat::Dxgi(DxgiFormat(70..=72))
    ) {
        return coverage.is_some_and(DdsAlphaCoverage::has_non_full_alpha);
    }
    observed.has_alpha
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use p4kx_extraction::{Archive, ArchiveTextures, PathIndex};
    use p4kx_testkit::{ArchiveBuilder, SplitDds, TestTextureFormat};

    use super::{DdsTextureFallback, unsupported};
    use crate::texture_encoding::TextureMode;

    #[test]
    fn blender_dds_capability_distinguishes_unsigned_data_from_signed_and_normals()
    -> Result<(), Box<dyn Error>> {
        let cases = [
            (83, TextureMode::Data, None),
            (
                83,
                TextureMode::Colour,
                Some(DdsTextureFallback::UnsupportedFormat),
            ),
            (
                83,
                TextureMode::Normal { gloss: false },
                Some(DdsTextureFallback::NormalCoordinateTransform),
            ),
            (84, TextureMode::Data, Some(DdsTextureFallback::SignedBc5)),
        ];
        let folder = tempfile::tempdir()?;
        let source = folder.path().join("Data.p4k");
        let mut builder = ArchiveBuilder::new();
        for code in [83, 84] {
            let dds = SplitDds::new(
                TestTextureFormat::Dx10Block {
                    dxgi_format: code,
                    block_bytes: 16,
                },
                7,
                5,
            );
            for entry in dds.entries(&format!("Data/textures/{code}.dds")) {
                builder = builder.entry(entry);
            }
        }
        builder.write_to(&source)?;
        let archive = Archive::open(&source)?;
        let index = PathIndex::new(&archive);
        let loader = ArchiveTextures::new(&index);
        for (code, mode, expected) in cases {
            let observed = loader.load_dds(&format!("textures/{code}.dds"), 1)?;
            assert_eq!(
                (observed.read.mip.width(), observed.read.mip.height()),
                (3, 2),
                "sub-block selected mip"
            );
            assert_eq!(
                unsupported(&observed.read, mode),
                expected,
                "DXGI {code}, role {mode:?}"
            );
        }
        Ok(())
    }
}
