// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_blender::{Manifest, TextureEncoding};
use p4kx_testkit::CheckedGlb;

use super::source::{FINISH_PALETTE_BYTE, LAYER_ONLY_CASES};
use crate::support::TestResult;

pub(super) fn verify_glb(glb: &CheckedGlb) -> TestResult {
    let scalar_roughness = 1.0 - f32::from(FINISH_PALETTE_BYTE) / 510.0;
    for case in &LAYER_ONLY_CASES {
        let material = glb
            .document
            .materials()
            .find(|row| row.name() == Some(case.name))
            .ok_or("child-only GLB material")?;
        let surface = material.pbr_metallic_roughness();
        assert!(
            surface.metallic_roughness_texture().is_none(),
            "{}: no sampled GLB roughness comes from the child",
            case.name,
        );
        assert!(
            (surface.roughness_factor() - scalar_roughness).abs() < 1e-6,
            "{}: current GLB palette fallback",
            case.name,
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
        assert!(
            parent.textures.is_empty(),
            "{}: no top-level texture",
            case.name
        );
        let [layer] = parent.layers.as_slice() else {
            return Err("one authored child material".into());
        };
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
