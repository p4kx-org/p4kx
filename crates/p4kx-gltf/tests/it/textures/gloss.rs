// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gltf::{LoadedTexture, TextureRole};
use p4kx_testkit::TestMtlMaterial;
use p4kx_texture::{
    DdsAlphaAssociation, DdsPngEncoding, DdsTransfer, DxgiFormat, RgbaImage, TextureFormat,
};

use super::support::{Stub, TestResult, export, material, packed_image, plain, texture, textured};

const COLOUR: &str = "Objects/Test/gloss_control_diff.dds";
const NORMAL: &str = "Objects/Test/sample_ddna.dds";
const MISSING_GLOSS: &str = "Objects/Test/absent_gloss.dds";
const GLOSS: &str = "Objects/Test/sample_gloss.dds";
const SPECULAR: &str = "Objects/Test/gloss_control_spec.dds";
const RGB_GLOSS_GREEN: u8 = 163;
const NORMAL_GLOSS_GREEN: u8 = 59;

fn authored(shader: &str) -> TestMtlMaterial {
    TestMtlMaterial::new("paint")
        .shader(shader)
        .diffuse([0.5; 3])
        .specular([1.0; 3])
        .texture("TexSlot1", COLOUR)
        .texture("TexSlot4", SPECULAR)
}

fn with_gloss(material: TestMtlMaterial) -> TestMtlMaterial {
    material
        .texture("TexSlot5", MISSING_GLOSS)
        .texture("TexSlot6", GLOSS)
}

fn inputs() -> Result<Stub, Box<dyn std::error::Error>> {
    let specular = LoadedTexture::new(
        RgbaImage::new(
            1,
            3,
            [64, 64, 64, 255, 177, 177, 177, 255, 225, 225, 225, 255].to_vec(),
        )?,
        false,
    );
    let gloss = texture([32, 96, 224, 17], true)?
        .with_dds_transfer(
            DdsTransfer::Srgb,
            DdsPngEncoding::of(TextureFormat::Dxgi(DxgiFormat(29)), DdsTransfer::Srgb),
        )
        .with_dds_alpha(DdsAlphaAssociation::Premultiplied, true);

    Ok(Stub::default()
        .with(COLOUR, &texture([128, 128, 128, 255], false)?)
        .with(SPECULAR, &specular)
        .with(GLOSS, &gloss)
        .sized())
}

#[test]
fn owned_hard_surface_uses_first_readable_explicit_gloss_without_resampling_specular() -> TestResult
{
    let loader = inputs()?;
    let (control, _) = export(
        &[plain("unused"), authored("HardSurface")],
        &loader,
        &textured(),
    )?;
    let control_image = packed_image(&control, &material(&control, "paint")?)?;
    let shader = with_gloss(authored("HardSurface")).texture("TexSlot2", NORMAL);
    let (glb, report) = export(&[plain("unused"), shader], &loader, &textured())?;
    let paint = material(&glb, "paint")?;
    let image = packed_image(&glb, &paint)?;
    assert_eq!(
        (image.width, image.height),
        (1, 3),
        "the specular dimensions"
    );
    for y in 0..3 {
        let [red, _, blue, alpha] = control_image.pixel(0, y).ok_or("control texel")?;
        assert_eq!(
            image.pixel(0, y),
            Some([red, RGB_GLOSS_GREEN, blue, alpha]),
            "only roughness changes at row {y}"
        );
    }
    assert_eq!(
        paint.pbr_metallic_roughness().roughness_factor().to_bits(),
        1.0_f32.to_bits(),
        "sampled green has unit roughness factor"
    );
    assert!(
        report
            .texture_fallbacks
            .iter()
            .any(|entry| { entry.role == TextureRole::Gloss && entry.file == MISSING_GLOSS }),
        "the unreadable earlier candidate is reported"
    );
    Ok(())
}

#[test]
fn owned_hard_surface_prefers_usable_normal_alpha_and_uses_gloss_after_alpha_loss() -> TestResult {
    for (has_alpha, expected) in [(true, NORMAL_GLOSS_GREEN), (false, RGB_GLOSS_GREEN)] {
        let normal = texture([128, 128, 255, 196], has_alpha)?;
        let loader = inputs()?.with(NORMAL, &normal);
        let shader = with_gloss(authored("HardSurface")).texture("TexSlot2", NORMAL);
        let (glb, _) = export(&[plain("unused"), shader], &loader, &textured())?;
        let image = packed_image(&glb, &material(&glb, "paint")?)?;
        assert_eq!(
            image.pixel(0, 0).map(|[_, green, _, _]| green),
            Some(expected),
            "only usable DDNA alpha overrides explicit gloss"
        );
        assert_eq!(
            loader
                .calls()
                .iter()
                .any(|(path, _)| path.eq_ignore_ascii_case(GLOSS)),
            !has_alpha,
            "explicit gloss decodes only when DDNA supplies no smoothness"
        );
    }
    Ok(())
}

#[test]
fn owned_hard_surface_exports_explicit_gloss_without_normal_or_specular() -> TestResult {
    let loader = inputs()?;
    let shader = TestMtlMaterial::new("paint")
        .shader("HardSurface")
        .texture("TexSlot1", COLOUR)
        .texture("TexSlot6", GLOSS);
    let (glb, _) = export(&[plain("unused"), shader], &loader, &textured())?;
    let paint = material(&glb, "paint")?;
    let image = packed_image(&glb, &paint)?;
    assert_eq!(
        (image.width, image.height),
        (2, 2),
        "the explicit gloss dimensions"
    );
    assert_eq!(
        image.pixel(0, 0),
        Some([255, RGB_GLOSS_GREEN, 255, 255]),
        "explicit gloss supplies roughness with an unused metalness channel"
    );
    Ok(())
}
