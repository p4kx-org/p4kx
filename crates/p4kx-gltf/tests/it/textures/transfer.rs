// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{CheckedGlb, TestMtlMaterial};
use p4kx_texture::{DdsPngEncoding, DdsTransfer, DxgiFormat, TextureFormat};
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
