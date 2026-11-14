// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The image format of a texture file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TextureFormat {
    /// An 8-bit RGBA PNG.
    #[serde(rename = "png")]
    Png,
    /// A standard DDS container holding an original selected mip.
    #[serde(rename = "dds")]
    Dds,
}

impl TextureFormat {
    pub(crate) const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Dds => "dds",
        }
    }
}
