// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::texture_codec::{
    DdsAlphaAssociation, DdsAlphaCoverage, DdsAlphaTransform, DdsPngTransform, DdsTransfer,
};
use p4kx_extraction::{TextureImageFace, TextureImageFormat};
use schemars::JsonSchema;
use serde::Serialize;

use super::family_inputs::{InputRefusal, MipBranch};
use crate::{Artifact, ReferenceUnavailable};

pub(super) const IMAGE: &str = "texture.dds-image";
pub(super) const CONVERSION: &str = "texture.dds-conversion";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", rename_all = "kebab-case")]
pub(super) enum ConversionPayload {
    Image(Image),
    Conversion(Conversion),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "TextureDdsImage")]
pub(super) struct Image {
    pub(super) level: u32,
    pub(super) layer: u32,
    pub(super) face: TextureImageFace,
    pub(super) slice: u32,
    pub(super) source_width: u32,
    pub(super) source_height: u32,
    pub(super) source_depth: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) format: ImageFormat,
    pub(super) bit_depth: u8,
    pub(super) suffix: String,
    pub(super) alpha: Alpha,
    pub(super) alpha_association: DdsAlphaAssociation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) alpha_coverage: Option<DdsAlphaCoverage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) alpha_transform: Option<DdsAlphaTransform>,
    pub(super) channel_transform: ChannelTransform,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source_transfer: Option<DdsTransfer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) output_transfer: Option<DdsTransfer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) png_transform: Option<DdsPngTransform>,
    pub(super) artifact: Artifact,
    pub(super) placement: Placement,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "TextureDdsImagePlacement")]
pub(super) enum Placement {
    Written,
    KeptIdentical,
}
#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "TextureDdsImageFormat")]
pub(super) enum ImageFormat {
    Png,
    OpenExr,
}

impl ImageFormat {
    pub(super) fn of(format: TextureImageFormat) -> Self {
        if format == TextureImageFormat::Exr {
            Self::OpenExr
        } else {
            Self::Png
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "TextureDdsImageAlpha")]
pub(super) enum Alpha {
    DecodedChannels,
    DecodedSplitChannel,
    NoAlphaChannel,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "TextureDdsChannelTransform")]
pub(super) enum ChannelTransform {
    Rgba8Decode,
    ReconstructedBc5Normal,
    LinearFloat,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "TextureDdsConversion")]
pub(super) struct Conversion {
    pub(super) requested_level: u32,
    pub(super) outcome: Outcome,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "TextureDdsConversionOutcome")]
pub(super) enum Outcome {
    Written { images: p4kx_extraction::DecimalU64 },
    Failed { failure: Failure },
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "TextureDdsConversionFailure")]
pub(super) enum Failure {
    InvalidHeader,
    UnsupportedDecoder,
    InconsistentLayout,
    UnavailableMip,
    Decode,
    Member {
        branch: Branch,
        part: Option<u32>,
        disposition: MemberDisposition,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "TextureDdsBranch")]
pub(super) enum Branch {
    Color,
    Alpha,
}

impl From<MipBranch> for Branch {
    fn from(branch: MipBranch) -> Self {
        match branch {
            MipBranch::Color => Self::Color,
            MipBranch::Alpha => Self::Alpha,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "TextureDdsConversionMember")]
pub(super) enum MemberDisposition {
    Missing,
    Ambiguous,
    Unproved { reason: ReferenceUnavailable },
    KeyRequired,
    WrongKey,
    Unreadable,
}

impl From<InputRefusal> for MemberDisposition {
    fn from(refusal: InputRefusal) -> Self {
        match refusal {
            InputRefusal::Missing => Self::Missing,
            InputRefusal::Ambiguous => Self::Ambiguous,
            InputRefusal::Unproved(reason) => Self::Unproved { reason },
            InputRefusal::KeyRequired => Self::KeyRequired,
            InputRefusal::WrongKey => Self::WrongKey,
            InputRefusal::Unreadable => Self::Unreadable,
        }
    }
}
