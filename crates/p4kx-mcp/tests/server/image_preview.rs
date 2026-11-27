// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use p4kx_testkit::{SplitDds, TestContainer, TestEntry, TestTextureFormat};
use serde_json::{Value, json};

use crate::support::{Install, Server, TestResult};

const TEXTURE: &str = "Data/Textures/test/checker.dds";
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

fn install() -> TestResult<Install> {
    let mut entries = SplitDds::new(TestTextureFormat::BC7_SRGB, 32, 16)
        .split(2)
        .entries(TEXTURE);
    entries.extend(
        SplitDds::new(TestTextureFormat::BC7_SRGB, 16, 16)
            .cubemap()
            .entries("Data/Textures/test/sky.dds"),
    );
    let pack = SplitDds::new(TestTextureFormat::BC7_SRGB, 8, 8)
        .entries("inner\\icon.dds")
        .into_iter()
        .fold(TestContainer::new(), TestContainer::entry);
    entries.push(TestEntry::stored(
        "Data\\Textures\\pack.socpak",
        pack.build(),
    ));
    entries.push(TestEntry::stored("Data/Textures/test/readme.txt", "hello"));
    Install::new(entries)
}

pub(super) fn preview(server: &mut Server, arguments: &Value) -> TestResult<(Value, Vec<u8>)> {
    let result = server.call("image_preview", arguments)?;
    assert_ne!(result.get("isError"), Some(&json!(true)), "{result}");
    let image = result
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|block| block.get("type") == Some(&json!("image")))
        .ok_or_else(|| format!("no image block: {result}"))?;
    assert_eq!(
        image.get("mimeType"),
        Some(&json!("image/png")),
        "the media type"
    );
    let data = image
        .get("data")
        .and_then(Value::as_str)
        .ok_or("image data")?;
    let meta = result.get("structuredContent").cloned().unwrap_or_default();
    Ok((meta, STANDARD.decode(data)?))
}

#[test]
fn a_texture_is_returned_as_a_png_image_block_with_its_metadata() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let (meta, png) = preview(
        &mut server,
        &json!({ "path": "data\\textures\\TEST\\checker.dds" }),
    )?;

    assert_eq!(png.get(..8), Some(&PNG_SIGNATURE[..]), "a PNG file");
    assert_eq!(meta["png_bytes"], png.len(), "its size");
    assert_eq!(meta["path"], TEXTURE, "the header Entry");
    assert_eq!(
        (
            meta["texture_width"].clone(),
            meta["texture_height"].clone()
        ),
        (json!(32), json!(16)),
        "the texture size"
    );
    assert_eq!(meta["mip"], 0, "the largest mip fits 512");
    Ok(())
}

#[test]
fn max_side_and_mip_choose_the_level() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let (small, _) = preview(&mut server, &json!({ "path": TEXTURE, "max_side": 8 }))?;
    let (chosen, _) = preview(&mut server, &json!({ "path": TEXTURE, "mip": 2 }))?;

    assert_eq!(
        (small["width"].clone(), small["height"].clone()),
        (json!(8), json!(4)),
        "fits 8"
    );
    assert_eq!(chosen["mip"], 2, "the mip asked for");
    assert_eq!(chosen["width"], 8, "its width");
    let error = server.error("image_preview", &json!({ "path": TEXTURE, "mip": 99 }))?;
    assert!(error.contains("does not exist"), "{error}");
    Ok(())
}

#[test]
fn a_cubemap_is_a_cross_or_one_face() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let sky = "Data/Textures/test/sky.dds";

    let (cross, _) = preview(&mut server, &json!({ "path": sky }))?;
    let (face, _) = preview(&mut server, &json!({ "path": sky, "face": "negz" }))?;
    let not_cube = server.error("image_preview", &json!({ "path": TEXTURE, "face": "posx" }))?;
    let bad = server.error("image_preview", &json!({ "path": sky, "face": "up" }))?;

    assert_eq!(
        (cross["width"].clone(), cross["height"].clone()),
        (json!(64), json!(48)),
        "4x3 faces"
    );
    assert_eq!(cross["cubemap"], true, "flagged");
    assert_eq!(face["face"], "negz", "the face");
    assert_eq!(
        (face["width"].clone(), face["height"].clone()),
        (json!(16), json!(16)),
        "one face"
    );
    assert!(not_cube.contains("no cubemap"), "{not_cube}");
    assert!(bad.contains("no cubemap face"), "{bad}");
    Ok(())
}

#[test]
fn a_texture_inside_a_container_is_previewed() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let (meta, png) = preview(
        &mut server,
        &json!({ "path": "Data/Textures/pack.socpak::inner/icon.dds" }),
    )?;

    assert_eq!(png.get(..8), Some(&PNG_SIGNATURE[..]), "a PNG file");
    assert_eq!(
        meta["path"], "Data\\Textures\\pack.socpak::inner\\icon.dds",
        "addressed with its container"
    );
    Ok(())
}

#[test]
fn unknown_and_non_texture_entries_are_errors() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let unknown = server.error("image_preview", &json!({ "path": "Data/none.dds" }))?;
    let text = server.error(
        "image_preview",
        &json!({ "path": "Data/Textures/test/readme.txt" }),
    )?;

    assert!(unknown.contains("holds no Entry"), "{unknown}");
    assert!(text.contains("cannot read the Entry"), "{text}");
    Ok(())
}

#[test]
fn a_mip_with_too_many_pixels_is_refused_and_the_automatic_level_avoids_it() -> TestResult {
    let big = SplitDds::new(TestTextureFormat::BC7_SRGB, 4096, 4096).entries("Data/big.dds");
    let install = Install::new(big)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let error = server.error(
        "image_preview",
        &json!({ "path": "Data/big.dds", "mip": 0 }),
    )?;
    let (chosen, _) = preview(&mut server, &json!({ "path": "Data/big.dds" }))?;

    assert!(error.contains("pixels"), "{error}");
    assert_eq!(chosen["mip"], 3, "the level that fits 512");
    Ok(())
}

#[test]
fn the_pixel_limit_counts_every_face_of_a_cubemap_in_face_mode() -> TestResult {
    let sky = SplitDds::new(TestTextureFormat::BC7_SRGB, 2048, 2048)
        .cubemap()
        .entries("Data/sky.dds");
    let install = Install::new(sky)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let error = server.error(
        "image_preview",
        &json!({ "path": "Data/sky.dds", "face": "posx", "mip": 0 }),
    )?;
    let (face, _) = preview(
        &mut server,
        &json!({ "path": "Data/sky.dds", "face": "posx" }),
    )?;

    assert!(error.contains("pixels"), "{error}");
    assert_eq!(face["width"], 512, "the automatic level fits");
    Ok(())
}
