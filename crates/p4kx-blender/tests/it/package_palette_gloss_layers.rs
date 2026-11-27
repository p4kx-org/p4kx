// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_blender::{Manifest, MaterialDescription, TextureEncoding, TextureUse};
use p4kx_testkit::CheckedGlb;

use super::source::{
    FINISH_PALETTE_BYTE, LAYER_ONLY_CASES, LayerOnlyCase, NORMAL_ALPHA, ROOT_AO_PATH,
};
use crate::support::TestResult;

pub(super) fn verify_glb(glb: &CheckedGlb) -> TestResult {
    let palette = f32::from(FINISH_PALETTE_BYTE) / 255.0;
    for case in &LAYER_ONLY_CASES {
        let material = glb
            .document
            .materials()
            .find(|row| row.name() == Some(case.name))
            .ok_or("child-only GLB material")?;
        let surface = material.pbr_metallic_roughness();
        let texture = surface
            .metallic_roughness_texture()
            .ok_or("child finish image")?;
        let image = glb.decode(&texture.texture().source())?;
        let [_, green, blue, _] = image.pixel(0, 0).ok_or("child finish texel")?;
        let smoothness = if case.slot == "TexSlot2" {
            f32::from(NORMAL_ALPHA) / 255.0
        } else {
            32.0 / 255.0
        };
        let expected = (smoothness * 0.5).mul_add(-palette, 1.0);
        assert!(
            (f32::from(green) / 255.0 - expected).abs() <= 0.5 / 255.0,
            "{}: child sampled palette finish",
            case.name
        );
        assert_eq!(blue, 255, "unused packed metalness stays neutral");
        assert_eq!(
            surface.roughness_factor(),
            1.0,
            "the selected finish is baked into the packed image"
        );
        assert_eq!(
            material.normal_texture().is_some(),
            case.slot == "TexSlot2",
            "{}: the child DDNA can still contribute a normal map",
            case.name,
        );
    }
    Ok(())
}

pub(super) fn verify_package(manifest: &Manifest) -> TestResult {
    for case in &LAYER_ONLY_CASES {
        let parent = manifest
            .materials
            .iter()
            .find(|row| row.name == case.name)
            .ok_or("child-only package material")?;
        let [root_slot] = parent.textures.as_slice() else {
            return Err("one authored wrapper occlusion texture".into());
        };
        assert_eq!(
            root_slot.slot, "TexSlot1",
            "the original wrapper slot is retained"
        );
        assert_eq!(
            root_slot.file, ROOT_AO_PATH,
            "the wrapper selects the actual AO source"
        );
        assert_eq!(
            root_slot.usage,
            TextureUse::Occlusion,
            "slot one with the AO suffix must not suppress the child finish"
        );
        verify_layer_shares(parent, case);
        let layer = parent.layers.first().ok_or("the authored primary child")?;
        assert_eq!(
            layer.gloss_mult.map(f32::to_bits),
            Some(0.5_f32.to_bits()),
            "{}: the authored layer multiplier is retained",
            case.name,
        );
        let child = manifest
            .materials
            .iter()
            .find(|row| Some(row.id.as_str()) == layer.material.as_deref())
            .ok_or("resolved child material")?;
        let [slot] = child.textures.as_slice() else {
            return Err("one authored child texture".into());
        };
        assert_eq!(
            (slot.slot.as_str(), slot.file.as_str()),
            (case.slot, case.path),
            "{}: the original child texture slot is retained",
            case.name,
        );
        let image = manifest
            .textures
            .iter()
            .find(|row| Some(row.id.as_str()) == slot.texture.as_deref())
            .ok_or("child texture image")?;
        assert_eq!(
            image.encoding,
            Some(TextureEncoding::Data),
            "{}: the child texture is data encoded",
            case.name,
        );
        super::verify_image_origin(image, case.slot == "TexSlot6")?;
    }
    Ok(())
}

fn verify_layer_shares(parent: &MaterialDescription, case: &LayerOnlyCase) {
    let expected_layers = if case.zero_wear_layer.is_some() { 2 } else { 1 };
    assert_eq!(
        parent.layers.len(),
        expected_layers,
        "both authored layer positions remain available"
    );
    if case.zero_wear_layer.is_some() {
        assert_eq!(
            parent.layer_shares,
            [1.0, 0.0],
            "explicit zero wear selects only the primary contribution"
        );
        assert!(
            parent.reads_vertex_colours,
            "the zero-wear case retains its vertex-mask declaration"
        );
        assert!(
            parent
                .parameters
                .iter()
                .any(|parameter| { parameter.name == "WearBlendBase" && parameter.value == "0" }),
            "the zero-wear token remains explicitly authored"
        );
    }
}
