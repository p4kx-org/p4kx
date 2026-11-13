// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{DdsTransfer, TextureFormat};

/// The RGB transfer and channel transform used by standalone DDS-to-PNG conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct DdsPngEncoding {
    /// Transfer of the PNG RGB samples, unknown when colour transfer does not apply.
    pub transfer: DdsTransfer,
    /// How the DDS samples became PNG samples.
    pub transform: DdsPngTransform,
}

/// Channel or transfer processing of a standalone DDS PNG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DdsPngTransform {
    /// Decoded colour samples retain their source transfer.
    PreserveColourTransfer,
    /// Float RGB is Reinhard tone-mapped and encoded as sRGB.
    ReinhardSrgb,
    /// BC5 reconstructs the normal's Z component.
    ReconstructedNormal,
    /// Alpha or integer data is represented without an RGB transfer declaration.
    DataChannels,
}

impl DdsPngEncoding {
    /// Determines the output of the existing DDS decoder from its exact source metadata.
    #[must_use]
    pub const fn of(format: TextureFormat, transfer: DdsTransfer) -> Self {
        if format.is_high_range() {
            return Self {
                transfer: DdsTransfer::Srgb,
                transform: DdsPngTransform::ReinhardSrgb,
            };
        }
        if let TextureFormat::Dxgi(format) = format {
            match format.0 {
                82..=84 => return Self::data(DdsPngTransform::ReconstructedNormal),
                3 | 4 | 7 | 8 | 12 | 14 | 17 | 18 | 25 | 30 | 32 | 36 | 38 | 42 | 43 | 50 | 52
                | 57 | 59 | 62 | 64 | 65 => return Self::data(DdsPngTransform::DataChannels),
                _ => {}
            }
        }
        Self {
            transfer,
            transform: DdsPngTransform::PreserveColourTransfer,
        }
    }

    /// Returns whether the output transfer depends on original colour declarations.
    #[must_use]
    pub const fn uses_source_transfer(self) -> bool {
        matches!(self.transform, DdsPngTransform::PreserveColourTransfer)
    }

    const fn data(transform: DdsPngTransform) -> Self {
        Self {
            transfer: DdsTransfer::Unknown,
            transform,
        }
    }
}
