// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::bc;
use crate::{DxgiFormat, Mip, TextureFormat};

const BLOCK_EDGE: usize = 4;

/// Exact alpha-value counts for one selected BC1, BC2 or BC3 image.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct DdsAlphaCoverage {
    /// In-bounds texels whose decoded alpha is zero.
    pub zero: u64,
    /// In-bounds texels whose decoded alpha is greater than zero and below full scale.
    pub partial: u64,
    /// In-bounds texels whose decoded alpha is full scale.
    pub full: u64,
}

impl DdsAlphaCoverage {
    /// Returns whether any in-bounds texel has less than full alpha.
    #[must_use]
    pub const fn has_non_full_alpha(self) -> bool {
        self.zero != 0 || self.partial != 0
    }

    const fn add(&mut self, value: u8) {
        match value {
            0 => self.zero += 1,
            u8::MAX => self.full += 1,
            _ => self.partial += 1,
        }
    }
}

impl Mip {
    /// Measures the original alpha samples of one image in this selected mip.
    ///
    /// Returns None for an unsupported encoding or absent surface. The result does not establish
    /// whether the material uses these samples for opacity or data.
    #[must_use]
    pub fn alpha_coverage(&self, surface: u64) -> Option<DdsAlphaCoverage> {
        let data = self.surface(surface)?;
        let width = self.width() as usize;
        let height = self.height() as usize;
        match self.header().format() {
            TextureFormat::Dxgi(DxgiFormat(70..=72)) => measure(data, width, height, bc::bc1_alpha),
            TextureFormat::Dxgi(DxgiFormat(73..=75)) => {
                measure(data, width, height, bc::bc2_block_alpha)
            }
            TextureFormat::Dxgi(DxgiFormat(76..=78)) => {
                measure(data, width, height, bc::bc3_block_alpha)
            }
            _ => None,
        }
    }
}

fn measure<const BYTES: usize>(
    data: &[u8],
    width: usize,
    height: usize,
    samples: impl Fn([u8; BYTES]) -> [u8; 16],
) -> Option<DdsAlphaCoverage> {
    let (blocks, remainder) = data.as_chunks::<BYTES>();
    let columns = width.div_ceil(BLOCK_EDGE);
    let rows = height.div_ceil(BLOCK_EDGE);
    if !remainder.is_empty() || blocks.len() != columns.checked_mul(rows)? {
        return None;
    }
    let mut coverage = DdsAlphaCoverage::default();
    for (index, block) in blocks.iter().enumerate() {
        let left = (index % columns) * BLOCK_EDGE;
        let top = (index / columns) * BLOCK_EDGE;
        for row in samples(*block)
            .as_chunks::<BLOCK_EDGE>()
            .0
            .iter()
            .take(height.saturating_sub(top))
        {
            for value in row.iter().take(width.saturating_sub(left)) {
                coverage.add(*value);
            }
        }
    }
    Some(coverage)
}
