// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::*;
use crate::surface_finish::SurfaceFinish;
use crate::texture_origins::TextureReadId;

fn source() -> LoadedTexture {
    LoadedTexture::new(
        RgbaImage::new(1, 1, vec![31, 97, 203, 128]).expect("one texel"),
        true,
    )
    .with_read(TextureReadId::new(7))
}

fn layer(at: usize, share: f32) -> LayerPlan {
    LayerPlan {
        at,
        share,
        gloss: Vec::new(),
        diffuse: None,
        normal: None,
        specular_map: None,
        finish: Some(SurfaceFinish::new(0.5, Some(0.8))),
        roughness: 0.6,
        colour: [1.0; 3],
        specular: [0.04; 3],
        metal: 0.0,
    }
}

#[test]
fn mask_selects_exact_sampled_layer_and_preserves_alpha_read_authority()
-> Result<(), Box<dyn std::error::Error>> {
    let texture = Arc::new(source());
    let inputs = vec![
        Input {
            selected: Some(Selected {
                texture: Arc::clone(&texture),
                repeats: [1.0; 2],
                channel: SmoothnessChannel::Alpha,
            }),
            selectors: Vec::new(),
        },
        Input::default(),
    ];
    let layered = Layered {
        layers: vec![layer(0, 0.5), layer(3, 0.5)],
        mask: None,
        refused: Vec::new(),
    };
    let mask = RgbaImage::new(1, 1, vec![255, 0, 0, 255])?;
    let maps = maps(&inputs, (1, 1))?;
    let pixels = bake(&layered, &maps, &Maps::default(), true, Some(&mask), (1, 1))?;
    assert_eq!((pixels.sampled, pixels.used), (1, 1));
    let expected = ((128.0_f32 / 255.0) * 0.5).mul_add(-0.8, 1.0);
    let green = texture_pixels::texel(&pixels.image, 0, 0)[1];
    assert!((f32::from(green) / 255.0 - expected).abs() <= 0.5 / 255.0);
    let uses = sources(&inputs, &[], None, &texture, &pixels);
    assert_eq!(uses.len(), 1);
    assert_eq!(uses[0].read, Some(TextureReadId::new(7)));
    assert_eq!(uses[0].components, TextureComponents::Alpha);
    Ok(())
}

#[test]
fn sampled_and_constant_layers_blend_finished_roughness_by_existing_shares()
-> Result<(), Box<dyn std::error::Error>> {
    let texture = source();
    let map = SmoothnessMap::new(&texture, [1.0; 2], (1, 1), SmoothnessChannel::Alpha)?;
    let layered = Layered {
        layers: vec![layer(0, 0.25), layer(3, 0.75)],
        mask: None,
        refused: Vec::new(),
    };
    let pixels = bake(
        &layered,
        &[Some(map), None],
        &Maps::default(),
        true,
        None,
        (1, 1),
    )?;
    assert_eq!((pixels.sampled, pixels.used), (1, 3));
    let sampled = ((128.0_f32 / 255.0) * 0.5).mul_add(-0.8, 1.0);
    let expected = sampled.mul_add(0.25, 0.6 * 0.75);
    let [red, green, blue, alpha] = texture_pixels::texel(&pixels.image, 0, 0);
    assert_eq!([red, blue, alpha], [255; 3]);
    assert!((f32::from(green) / 255.0 - expected).abs() <= 0.5 / 255.0);
    Ok(())
}

#[test]
fn red_and_alpha_reach_distinct_finished_layer_pixels_after_minification()
-> Result<(), Box<dyn std::error::Error>> {
    let pixels = [0_u8, 64, 128, 255]
        .into_iter()
        .flat_map(|alpha| [37, 89, 201, alpha])
        .collect();
    let texture = LoadedTexture::new(RgbaImage::new(2, 2, pixels)?, true);
    let layered = Layered {
        layers: vec![layer(0, 1.0)],
        mask: None,
        refused: Vec::new(),
    };
    for (channel, scalar) in [
        (SmoothnessChannel::Red, 37.0),
        (SmoothnessChannel::Alpha, 112.0),
    ] {
        let map = SmoothnessMap::new(&texture, [1.0; 2], (1, 1), channel)?;
        let pixels = bake(&layered, &[Some(map)], &Maps::default(), true, None, (1, 1))?;
        let green = texture_pixels::texel(&pixels.image, 0, 0)[1];
        let expected = (scalar / 255.0_f32 * 0.5).mul_add(-0.8, 1.0);
        assert!((texture_pixels::unit(green) - expected).abs() <= 0.5 / 255.0);
    }
    Ok(())
}
