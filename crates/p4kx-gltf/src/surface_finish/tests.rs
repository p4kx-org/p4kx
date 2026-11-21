// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::SurfaceFinish;
use crate::texture_pixels::{Smoothness, metallic_roughness, texel};
use p4kx_texture::RgbaImage;

fn fixtures() -> Vec<Value> {
    let value: Value = serde_json::from_str(include_str!(
        "../../../../blender_addon/tests/fixtures/palette_finish.json"
    ))
    .expect("synthetic shared finish fixture");
    value["cases"].as_array().expect("finish cases").clone()
}

fn number(value: &Value, key: &str) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "synthetic fixture values fit f32"
    )]
    let number = value[key].as_f64().expect("numeric fixture field") as f32;
    number
}

fn finish(case: &Value) -> SurfaceFinish {
    SurfaceFinish {
        multiplier: number(case, "multiplier").to_bits(),
        palette: (!case["paletteByte"].is_null())
            .then(|| (number(case, "paletteByte") / 255.0).to_bits()),
    }
}

#[test]
fn shared_finish_vectors_keep_palette_and_authored_multiplier_in_smoothness_space() {
    for case in fixtures() {
        let actual = finish(&case).sampled_roughness(number(&case, "smoothness"));
        assert!((actual - number(&case, "roughness")).abs() < 1e-6, "{case}");
    }
}

#[test]
fn packed_finish_vectors_change_only_green_and_keep_unit_metalness_channel() {
    for case in fixtures() {
        let smoothness = number(&case, "smoothness");
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "synthetic smoothness is in the unit interval"
        )]
        let alpha = (smoothness * 255.0).round() as u8;
        let input = RgbaImage::new(1, 1, vec![31, 97, 203, alpha]).expect("one texel");
        let packed = metallic_roughness(
            Some(Smoothness::NormalAlpha(&input)),
            None,
            [1.0; 3],
            Some(finish(&case)),
        )
        .expect("image")
        .expect("smoothness image");
        let [red, green, blue, alpha] = texel(&packed.image, 0, 0);
        assert_eq!([red, blue, alpha], [255; 3], "{case}");
        assert!(
            (f32::from(green) / 255.0 - number(&case, "roughness")).abs() <= 1.5 / 255.0,
            "{case}: {green}"
        );
        assert!(packed.roughness && !packed.metallic);
    }
}
