// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{DdsRawHeader, DxgiFormat};

const CRY_MARKER: u32 = u32::from_le_bytes(*b"FYRC");
const CRY_IMAGE_FLAGS: usize = 1;
const CRY_SRGB_READ: u32 = 0x800;

/// Transfer function declared for DDS colour samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum DdsTransfer {
    /// A linear transfer function.
    Linear,
    /// The sRGB transfer function.
    Srgb,
    /// The source does not establish a transfer function.
    Unknown,
}

/// Independent transfer declarations and their agreement in one authored DDS header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct DdsTransferEvidence {
    /// Transfer declared by the original typed DXGI format, when known.
    pub dxgi: DdsTransfer,
    /// The original Cry marker word, including unrecognised values.
    #[schemars(range(max = u32::MAX))]
    pub cry_marker: u32,
    /// The original image flags word, including unrecognised bits.
    #[schemars(range(max = u32::MAX))]
    pub cry_image_flags: u32,
    /// Transfer indicated by the sRGB-read bit when the marker is valid.
    pub cry: DdsTransfer,
    /// The transfer established by the compatible declarations.
    pub effective: DdsTransfer,
    /// Two recognised declarations disagree.
    pub conflict: bool,
}

impl DdsRawHeader {
    /// Interprets the original DXGI and marker-gated Cry transfer declarations.
    #[must_use]
    pub fn transfer(&self) -> DdsTransferEvidence {
        let legacy = self.legacy();
        let cry_marker = legacy.reserved2;
        let cry_image_flags = legacy.reserved1[CRY_IMAGE_FLAGS];
        let cry = match self.has_cry_image_flags() {
            false => DdsTransfer::Unknown,
            true if cry_image_flags & CRY_SRGB_READ == 0 => DdsTransfer::Linear,
            true => DdsTransfer::Srgb,
        };
        let dxgi = self.dx10().map_or(DdsTransfer::Unknown, |header| {
            DxgiFormat(header.dxgi_format).declared_transfer()
        });
        let (effective, conflict) = reconcile(dxgi, cry);
        DdsTransferEvidence {
            dxgi,
            cry_marker,
            cry_image_flags,
            cry,
            effective,
            conflict,
        }
    }

    pub(crate) const fn has_cry_image_flags(&self) -> bool {
        self.legacy().reserved2 == CRY_MARKER
    }
}

impl DxgiFormat {
    /// Returns the transfer declared by a recognised typed DXGI colour or sampled-data format.
    #[must_use]
    pub const fn declared_transfer(self) -> DdsTransfer {
        match self.0 {
            29 | 72 | 75 | 78 | 91 | 93 | 99 => DdsTransfer::Srgb,
            2 | 6 | 10 | 11 | 13 | 16 | 24 | 26 | 28 | 31 | 34 | 35 | 37 | 41 | 49 | 51 | 54
            | 56 | 58 | 61 | 63 | 65 | 67 | 71 | 74 | 77 | 80 | 81 | 83 | 84 | 85 | 86 | 87
            | 88 | 95 | 96 | 98 | 115 => DdsTransfer::Linear,
            _ => DdsTransfer::Unknown,
        }
    }
}

const fn reconcile(dxgi: DdsTransfer, cry: DdsTransfer) -> (DdsTransfer, bool) {
    use DdsTransfer::{Linear, Srgb, Unknown};
    match (dxgi, cry) {
        (Unknown, transfer) | (transfer, Unknown) => (transfer, false),
        (Linear, Linear) => (Linear, false),
        (Srgb, Srgb) => (Srgb, false),
        (Linear, Srgb) | (Srgb, Linear) => (Unknown, true),
    }
}
