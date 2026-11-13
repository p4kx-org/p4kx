// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gltf::TextureFile;
use p4kx_texture::{
    DdsAlphaAssociation, DdsAlphaTransform, DdsPngEncoding, DdsPngTransform, DdsTransfer,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ColourSpace, TextureUse};

/// Channel handling applied after decoding a DDS image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextureChannelTransform {
    /// The decoded channels are used in their original order.
    Preserve,
    /// The normal Y component is flipped to the consumer's coordinate convention.
    FlipNormalGreen,
}

/// How the package importer samples the image's colour channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextureEncoding {
    /// RGB uses the sRGB transfer function.
    Srgb,
    /// RGB carries scene-linear colour in Rec.709 primaries.
    LinearColour,
    /// Values are linear shader inputs without a colour-space transform.
    Data,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TextureMode {
    Colour,
    Specular,
    Data,
    Normal { gloss: bool },
}

impl TextureMode {
    pub(crate) const fn of(role: TextureUse, gloss: bool) -> Self {
        match role {
            TextureUse::Diffuse => Self::Colour,
            TextureUse::Specular => Self::Specular,
            TextureUse::Normal => Self::Normal { gloss },
            _ => Self::Data,
        }
    }

    pub(crate) const fn drawn(file: &TextureFile, role: TextureUse) -> Self {
        if file.opacity {
            Self::Data
        } else {
            Self::of(role, file.gloss)
        }
    }

    pub(crate) const fn encoding(self, transfer: DdsTransfer) -> TextureEncoding {
        match (self, transfer) {
            (Self::Colour | Self::Specular, DdsTransfer::Linear) => TextureEncoding::LinearColour,
            (Self::Colour, _) | (Self::Specular, DdsTransfer::Srgb) => TextureEncoding::Srgb,
            _ => TextureEncoding::Data,
        }
    }

    pub(crate) const fn tag(self, transfer: DdsTransfer) -> DdsTransfer {
        match self {
            Self::Colour | Self::Specular => transfer,
            _ => DdsTransfer::Unknown,
        }
    }

    pub(crate) const fn origin_encoding(self, encoding: DdsPngEncoding) -> DdsPngEncoding {
        match self {
            Self::Colour | Self::Specular => encoding,
            _ if !matches!(encoding.transform, DdsPngTransform::PreserveColourTransfer) => encoding,
            _ => DdsPngEncoding::data_channels(),
        }
    }

    pub(crate) const fn channel_transform(self) -> TextureChannelTransform {
        if matches!(self, Self::Normal { .. }) {
            TextureChannelTransform::FlipNormalGreen
        } else {
            TextureChannelTransform::Preserve
        }
    }

    pub(crate) const fn uses_colour_samples(self) -> bool {
        matches!(self, Self::Colour | Self::Specular)
    }

    pub(crate) const fn opacity_transform(
        self,
        association: DdsAlphaAssociation,
        transfer: DdsTransfer,
    ) -> DdsAlphaTransform {
        if self.uses_colour_samples() {
            DdsAlphaTransform::for_opacity(association, transfer)
        } else {
            DdsAlphaTransform::Preserve
        }
    }

    pub(crate) const fn uses_alpha(
        self,
        has_alpha: bool,
        association: DdsAlphaAssociation,
        split_alpha: bool,
    ) -> bool {
        has_alpha
            && (split_alpha
                || !self.uses_colour_samples()
                || !matches!(
                    association,
                    DdsAlphaAssociation::Opaque | DdsAlphaAssociation::Custom
                ))
    }

    pub(crate) const fn legacy_space(self) -> ColourSpace {
        if matches!(self, Self::Colour) {
            ColourSpace::Srgb
        } else {
            ColourSpace::Linear
        }
    }
}
