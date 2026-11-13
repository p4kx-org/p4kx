// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::pixels::float_unorm;
use crate::{DdsAlphaAssociation, DdsTransfer, RgbaImage, TextureError};

/// Alpha-associated colour processing for an output image.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DdsAlphaTransform {
    /// Retain the decoded colour and alpha samples.
    #[default]
    Preserve,
    /// Divide linear-light colour by alpha and retain the established transfer.
    UnassociateLinear,
    /// Divide the stored sample values by alpha when the source transfer is unknown.
    UnassociateStored,
}

impl DdsAlphaTransform {
    /// Chooses the colour transform for an output that uses alpha as opacity.
    #[must_use]
    pub const fn for_opacity(association: DdsAlphaAssociation, transfer: DdsTransfer) -> Self {
        match (association, transfer) {
            (DdsAlphaAssociation::Premultiplied, DdsTransfer::Unknown) => Self::UnassociateStored,
            (DdsAlphaAssociation::Premultiplied, _) => Self::UnassociateLinear,
            _ => Self::Preserve,
        }
    }

    /// Applies the transform to RGB without changing the supplied alpha sample.
    #[must_use]
    pub fn texel(self, transfer: DdsTransfer, pixel: [u8; 4]) -> [u8; 4] {
        self.texel_with_opacity(transfer, pixel, pixel[3])
    }

    /// Applies the transform using the original opacity before an alpha-tail merge.
    #[must_use]
    pub fn texel_with_opacity(
        self,
        transfer: DdsTransfer,
        [red, green, blue, alpha]: [u8; 4],
        opacity: u8,
    ) -> [u8; 4] {
        if self == Self::Preserve || opacity == u8::MAX {
            return [red, green, blue, alpha];
        }
        if opacity == 0 {
            return [0, 0, 0, alpha];
        }
        let opacity = f32::from(opacity) / f32::from(u8::MAX);
        let straight = |sample| {
            let encoded = f32::from(sample) / f32::from(u8::MAX);
            let value = if self == Self::UnassociateLinear && transfer == DdsTransfer::Srgb {
                to_srgb(from_srgb(encoded) / opacity)
            } else {
                encoded / opacity
            };
            float_unorm(value)
        };
        [straight(red), straight(green), straight(blue), alpha]
    }

    /// Applies the transform in place to every pixel of one decoded image.
    pub fn image(self, transfer: DdsTransfer, image: &mut RgbaImage) {
        if self == Self::Preserve {
            return;
        }
        for pixel in image.pixels_mut().as_chunks_mut::<4>().0 {
            *pixel = self.texel(transfer, *pixel);
        }
    }

    /// Applies the transform with the opacity from the original colour surface.
    ///
    /// # Errors
    /// Returns a length error if opacity does not have one sample per output texel.
    pub fn image_with_opacity(
        self,
        transfer: DdsTransfer,
        image: &mut RgbaImage,
        opacity: &[u8],
    ) -> Result<(), TextureError> {
        let pixels = image.pixels_mut().as_chunks_mut::<4>().0;
        if pixels.len() != opacity.len() {
            return Err(TextureError::MipLength {
                expected: pixels.len() as u64,
                actual: opacity.len() as u64,
            });
        }
        for (pixel, opacity) in pixels.iter_mut().zip(opacity) {
            *pixel = self.texel_with_opacity(transfer, *pixel, *opacity);
        }
        Ok(())
    }
}

fn from_srgb(value: f32) -> f32 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055_f32.mul_add(value.powf(1.0 / 2.4), -0.055)
    }
}

#[cfg(test)]
mod tests {
    use super::DdsAlphaTransform;
    use crate::{DdsAlphaAssociation, DdsTransfer};

    #[test]
    fn straightening_is_explicit_about_transfer_and_zero_alpha() {
        use DdsAlphaAssociation::{Premultiplied, Straight};
        use DdsAlphaTransform::{Preserve, UnassociateLinear, UnassociateStored};
        use DdsTransfer::{Linear, Srgb, Unknown};

        let rgba = [64, 32, 128, 128];
        assert_eq!(
            DdsAlphaTransform::for_opacity(Premultiplied, Unknown),
            UnassociateStored,
            "untagged colour keeps its sample domain"
        );
        assert_eq!(
            DdsAlphaTransform::for_opacity(Premultiplied, Srgb),
            UnassociateLinear,
            "tagged sRGB is straightened in light"
        );
        assert_eq!(
            DdsAlphaTransform::for_opacity(Straight, Linear),
            Preserve,
            "straight input is unchanged"
        );
        assert_eq!(
            UnassociateStored.texel(Unknown, rgba),
            [128, 64, 255, 128],
            "sample-domain unassociation"
        );
        assert_eq!(
            UnassociateLinear.texel(Linear, rgba),
            [128, 64, 255, 128],
            "linear colour unassociation"
        );
        assert_eq!(
            UnassociateLinear.texel(Srgb, rgba),
            [90, 47, 175, 128],
            "sRGB transfer before and after division"
        );
        assert_eq!(
            UnassociateLinear.texel(Srgb, [64, 32, 128, 0]),
            [0; 4],
            "zero alpha maps to black"
        );
        assert_eq!(
            UnassociateLinear.texel(Srgb, [64, 32, 128, 255]),
            [64, 32, 128, 255],
            "opaque texels are byte-identical"
        );
    }
}
