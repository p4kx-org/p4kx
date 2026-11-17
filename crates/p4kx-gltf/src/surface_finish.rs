// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{Material, MaterialLayer, PaletteSlot};

const MAX_GLOSSINESS: f32 = 255.0;
const FALLBACK_SMOOTHNESS: f32 = 0.5;

/// The P4KX finish controls selected for one sampled surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SurfaceFinish {
    multiplier: u32,
    palette: Option<u32>,
}

impl SurfaceFinish {
    pub(crate) fn new(multiplier: f32, palette: Option<f32>) -> Self {
        Self {
            multiplier: multiplier.to_bits(),
            palette: palette.map(f32::to_bits),
        }
    }

    pub(crate) fn layer(layer: &MaterialLayer) -> Self {
        Self::new(
            layer.gloss_mult(),
            layer
                .palette_entry
                .map(|entry| normalized(entry.glossiness)),
        )
    }

    pub(crate) fn material(material: &Material) -> Option<Self> {
        (material.layers.is_empty()
            && material.shader.eq_ignore_ascii_case("HardSurface")
            && material.tinted
            && matches!(
                material.palette,
                Some(PaletteSlot::Primary | PaletteSlot::Secondary | PaletteSlot::Tertiary)
            ))
        .then(|| Self::new(1.0, Some(normalized(material.shininess))))
    }

    pub(crate) fn sampled_roughness(self, smoothness: f32) -> f32 {
        let palette = self.palette.map_or(1.0, f32::from_bits);
        (smoothness * f32::from_bits(self.multiplier))
            .mul_add(-palette, 1.0)
            .clamp(0.0, 1.0)
    }

    pub(crate) fn fallback_roughness(self) -> f32 {
        let smoothness = self.palette.map_or(FALLBACK_SMOOTHNESS, f32::from_bits);
        smoothness
            .mul_add(-f32::from_bits(self.multiplier), 1.0)
            .clamp(0.0, 1.0)
    }

    pub(crate) fn is_neutral(self) -> bool {
        self.multiplier == 1.0_f32.to_bits()
            && self.palette.is_none_or(|value| value == 1.0_f32.to_bits())
    }
}

fn normalized(glossiness: f32) -> f32 {
    (glossiness / MAX_GLOSSINESS).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests;
