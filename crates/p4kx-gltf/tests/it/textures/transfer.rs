// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{CheckedGlb, TestMtlMaterial};
use p4kx_texture::{DdsAlphaAssociation, DdsPngEncoding, DdsTransfer, DxgiFormat, TextureFormat};
use serde_json::{Value, json};

use super::support::{
    Stub, TestResult, base_image, export, image_of, material, packed_image, plain, texture,
    textured,
};
use crate::layer_textures;

const COLOUR: &str = "Objects/Test/plate_diff.dds";
const NORMAL: &str = "Objects/Test/plate_ddna.dds";
const TEXEL: [u8; 4] = [128, 64, 16, 81];
const SRGB: [u8; 4] = [188, 137, 71, 81];

#[test]
fn colour_normal_and_smoothness_keep_distinct_transfer_roles() -> TestResult {
    let map = texture(TEXEL, true)?.with_dds_transfer(
        DdsTransfer::Linear,
        DdsPngEncoding::of(TextureFormat::Dxgi(DxgiFormat(28)), DdsTransfer::Linear),
    );
    let stub = Stub::default().with(COLOUR, &map).with(NORMAL, &map);
    let authored = TestMtlMaterial::new("plate")
        .diffuse([1.0; 3])
        .opacity(0.5)
        .texture("TexSlot1", COLOUR)
        .texture("TexSlot2", NORMAL);
    let (glb, _) = export(&[plain("unused"), authored], &stub, &textured())?;
    assert_eq!(
        stub.calls().len(),
        2,
        "normal and smoothness share one physical read"
    );
    let plate = material(&glb, "plate")?;
    assert_eq!(
        base_image(&glb, &plate)?.pixel(0, 0),
        Some(SRGB),
        "RGB changes, alpha remains linear"
    );
    let normal = plate
        .normal_texture()
        .map(|info| info.texture())
        .ok_or("normal image")?;
    assert_eq!(
        image_of(&glb, Some(normal.clone()))?.pixel(0, 0),
        Some([128, 191, 16, 255]),
        "data bytes follow only the normal coordinate transform"
    );
    assert_eq!(
        packed_image(&glb, &plate)?.pixel(0, 0),
        Some([255, 174, 255, 255]),
        "smoothness inverse remains linear"
    );
    let base = plate
        .pbr_metallic_roughness()
        .base_color_texture()
        .ok_or("base image")?
        .texture();
    let source = base.source();
    check_roles(&glb, &source, &normal.source())
}

fn check_roles(
    glb: &CheckedGlb,
    source: &p4kx_testkit::gltf::Image<'_>,
    normal: &p4kx_testkit::gltf::Image<'_>,
) -> TestResult {
    let colour_extras = transfer(source)?;
    let normal_extras = transfer(normal)?;
    for (extras, field, value) in [
        (&colour_extras, "/output", json!("srgb")),
        (&colour_extras, "/inputs/0/declared", json!("linear")),
        (
            &colour_extras,
            "/inputs/0/colour_encoding/transfer",
            json!("linear"),
        ),
        (&normal_extras, "/output", json!("linear")),
        (&normal_extras, "/inputs/0/colour_encoding", Value::Null),
    ] {
        assert_eq!(
            extras.pointer(field),
            Some(&value),
            "transfer field {field}"
        );
    }
    assert_png_tag(glb, source, true)?;
    assert_png_tag(glb, normal, false)?;
    Ok(())
}

#[test]
fn a_linear_layer_decodes_before_blending_and_encodes_srgb() -> TestResult {
    let authored = layer_textures::hull("hull", false, 1, "0");
    let files = vec![("layers/l0", layer_textures::layer_file("a_diff", None))];
    let map = layer_textures::uniform(TEXEL)?.with_dds_transfer(
        DdsTransfer::Linear,
        DdsPngEncoding::of(TextureFormat::Dxgi(DxgiFormat(28)), DdsTransfer::Linear),
    );
    let stub = Stub::default().with("objects/test/a_diff.dds", &map);
    let (glb, _) = layer_textures::export(&[plain("unused"), authored], files, &stub)?;
    assert_eq!(
        layer_textures::base_of(&glb, "hull")?.pixel(0, 0),
        Some([188, 137, 71, 255]),
        "linear layer is encoded as sRGB without altering alpha conventions"
    );
    Ok(())
}

#[test]
fn premultiplied_colour_is_straight_in_glb_and_packed_alpha_is_preserved() -> TestResult {
    let rgba = [64, 32, 128, 128];
    let map = texture(rgba, true)?
        .with_dds_transfer(
            DdsTransfer::Linear,
            DdsPngEncoding::of(TextureFormat::Dxgi(DxgiFormat(28)), DdsTransfer::Linear),
        )
        .with_dds_alpha(DdsAlphaAssociation::Premultiplied, true);
    let stub = Stub::default().with(COLOUR, &map).with(NORMAL, &map);
    let authored = TestMtlMaterial::new("plate")
        .diffuse([1.0; 3])
        .opacity(0.5)
        .texture("TexSlot1", COLOUR)
        .texture("TexSlot2", NORMAL);
    let (glb, _) = export(&[plain("unused"), authored], &stub, &textured())?;
    let plate = material(&glb, "plate")?;
    assert_eq!(
        plate.alpha_mode(),
        p4kx_testkit::gltf::material::AlphaMode::Blend,
        "material still determines blending"
    );
    assert_eq!(
        base_image(&glb, &plate)?.pixel(0, 0),
        Some([188, 137, 255, 128]),
        "base colour is straight and sRGB"
    );
    assert_eq!(
        packed_image(&glb, &plate)?.pixel(0, 0),
        Some([255, 127, 255, 255]),
        "normal smoothness remains packed"
    );
    let base = plate
        .pbr_metallic_roughness()
        .base_color_texture()
        .ok_or("colour image")?
        .texture()
        .source();
    let normal = plate
        .normal_texture()
        .ok_or("normal map")?
        .texture()
        .source();
    assert_eq!(
        transfer(&base)?.pointer("/inputs/0/alpha_association"),
        Some(&json!("premultiplied")),
        "source declaration"
    );
    assert_eq!(
        transfer(&base)?.pointer("/inputs/0/alpha_transform"),
        Some(&json!("unassociate_linear")),
        "base image transformation"
    );
    assert_eq!(
        transfer(&normal)?.pointer("/inputs/0/alpha_transform"),
        None,
        "normal is a data role"
    );
    Ok(())
}

#[test]
fn split_alpha_keeps_its_output_opacity_and_uses_embedded_opacity_for_colour() -> TestResult {
    let map = texture([64, 32, 128, 64], true)?
        .with_dds_transfer(
            DdsTransfer::Linear,
            DdsPngEncoding::of(TextureFormat::Dxgi(DxgiFormat(28)), DdsTransfer::Linear),
        )
        .with_dds_alpha(DdsAlphaAssociation::Premultiplied, true)
        .with_split_alpha(Some(vec![128; 4]))?;
    let stub = Stub::default().with(COLOUR, &map);
    let authored = TestMtlMaterial::new("plate")
        .diffuse([1.0; 3])
        .opacity(0.5)
        .texture("TexSlot1", COLOUR);
    let (glb, _) = export(&[plain("unused"), authored], &stub, &textured())?;
    let plate = material(&glb, "plate")?;
    assert_eq!(
        base_image(&glb, &plate)?.pixel(0, 0),
        Some([188, 137, 255, 64]),
        "unassociation divides by 128, and the output keeps the merged opacity 64"
    );
    Ok(())
}

#[test]
fn declared_custom_alpha_does_not_become_base_colour_opacity() -> TestResult {
    let map = texture(TEXEL, true)?.with_dds_alpha(DdsAlphaAssociation::Custom, true);
    let stub = Stub::default().with(COLOUR, &map);
    let authored = TestMtlMaterial::new("plate")
        .opacity(0.5)
        .texture("TexSlot1", COLOUR);
    let (glb, _) = export(&[plain("unused"), authored], &stub, &textured())?;
    let plate = material(&glb, "plate")?;
    assert_eq!(
        plate.alpha_mode(),
        p4kx_testkit::gltf::material::AlphaMode::Blend,
        "authored blend policy is independent"
    );
    assert_eq!(
        base_image(&glb, &plate)?.pixel(0, 0),
        Some([128, 64, 16, 255]),
        "custom data is not opacity"
    );
    Ok(())
}

fn transfer(image: &p4kx_testkit::gltf::Image<'_>) -> Result<Value, Box<dyn std::error::Error>> {
    let extras = image.extras().as_ref().ok_or("embedded image extras")?;
    let value: Value = serde_json::from_str(extras.get())?;
    value
        .get("p4kx_transfer")
        .cloned()
        .ok_or_else(|| "transfer extras".into())
}

fn assert_png_tag(
    glb: &CheckedGlb,
    image: &p4kx_testkit::gltf::Image<'_>,
    srgb: bool,
) -> TestResult {
    let png = glb.png(image)?;
    assert_eq!(
        png.get(37..41).map(|tag| json!(tag == b"sRGB")),
        Some(json!(srgb)),
        "embedded PNG chunk transfer agrees with its role"
    );
    Ok(())
}
