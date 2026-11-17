// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::RgbaImage;

use super::{SmoothnessChannel, SmoothnessMap};
use crate::texture_loader::LoadedTexture;

#[test]
fn alpha_minification_preserves_varying_smoothness_without_changing_rgb()
-> Result<(), Box<dyn std::error::Error>> {
    let pixels = [0_u8, 64, 128, 255]
        .into_iter()
        .flat_map(|alpha| [37, 89, 201, alpha])
        .collect();
    let source = LoadedTexture::new(RgbaImage::new(2, 2, pixels)?, true);
    let map = SmoothnessMap::new(&source, [1.0; 2], (1, 1), SmoothnessChannel::Alpha)?;
    assert_eq!(
        map.tile.image().pixels(),
        [37, 89, 201, 112],
        "the alpha consumer retains linear alpha and all RGB data"
    );
    assert!(
        (map.sample((0.5, 0.5)) - 112.0 / 255.0).abs() < 1e-6,
        "the finish samples alpha rather than an RGB surrogate"
    );
    Ok(())
}

#[test]
fn explicit_gloss_uses_red_and_does_not_consume_alpha() -> Result<(), Box<dyn std::error::Error>> {
    let pixels = [0_u8, 64, 128, 255]
        .into_iter()
        .flat_map(|alpha| [37, 89, 201, alpha])
        .collect();
    let source = LoadedTexture::new(RgbaImage::new(2, 2, pixels)?, true);
    let map = SmoothnessMap::new(&source, [1.0; 2], (1, 1), SmoothnessChannel::Red)?;
    let expected = 37.0 / 255.0;
    assert!(
        (map.sample((0.5, 0.5)) - expected).abs() < 1e-6,
        "explicit scalar gloss reads red independently of green, blue and alpha"
    );
    assert_eq!(
        map.tile.image().pixels(),
        [37, 89, 201, 255],
        "the existing RGB tile mode stays opaque"
    );
    Ok(())
}
