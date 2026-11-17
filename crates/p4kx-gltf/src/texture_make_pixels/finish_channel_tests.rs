// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::RgbaImage;

use super::PackedInputs;
use crate::surface_finish::SurfaceFinish;
use crate::texture_loader::LoadedTexture;
use crate::texture_pixels::Smoothness;

#[test]
fn selected_own_finish_chooses_red_and_generic_gloss_keeps_rgb() {
    let image = RgbaImage::new(1, 1, vec![37, 89, 201, 112]).expect("one valid texel");
    let texture = LoadedTexture::new(image, true);
    let inputs = PackedInputs {
        normal: None,
        gloss: Some(&texture),
        specular: None,
    };
    assert!(matches!(
        inputs.smoothness(Some(SurfaceFinish::new(1.0, Some(0.8)))),
        Some(Smoothness::ExplicitRed(_))
    ));
    assert!(matches!(
        inputs.smoothness(None),
        Some(Smoothness::ExplicitRgb(_))
    ));
    let inputs = PackedInputs {
        normal: Some(&texture),
        gloss: Some(&texture),
        specular: None,
    };
    assert!(matches!(
        inputs.smoothness(Some(SurfaceFinish::new(1.0, Some(0.8)))),
        Some(Smoothness::NormalAlpha(_))
    ));
}
