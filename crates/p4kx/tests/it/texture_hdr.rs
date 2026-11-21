// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx texture png` on high-range BC6H textures: OpenEXR by default, 8-bit PNG with
//! `--tone-map`.

use std::error::Error;
use std::fs::File;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use exr::prelude::{ReadChannels, ReadLayers, Vec2};
use p4kx_testkit::{SplitDds, TestTextureFormat};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn Error>>;

fn p4kx() -> Result<Command, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?)
}

/// A mode 11 block with 10-bit endpoints `first` and `second` in every channel, index 0 for
/// texel 0, 8 for texel 1, 15 for texel 15 and 0 for the others.
fn mode_11(first: u128, second: u128) -> [u8; 16] {
    let endpoints = (0..3).fold(0, |bits, channel| {
        bits | (first << (5 + 10 * channel)) | (second << (35 + 10 * channel))
    });
    (0b00011 | endpoints | (8 << 68) | (15 << 124)).to_le_bytes()
}

/// Writes a 4×4 texture of one mip in DXGI format `dxgi_format` holding `block`.
fn write_block(
    folder: &Path,
    dxgi_format: u32,
    block: [u8; 16],
) -> Result<PathBuf, Box<dyn Error>> {
    let format = TestTextureFormat::Dx10Block {
        dxgi_format,
        block_bytes: 16,
    };
    let dds = SplitDds::new(format, 4, 4).mips(1);
    let mut bytes = dds.header_bytes();
    bytes.truncate(dds.header_len());
    bytes.extend_from_slice(&block);
    let path = folder.join("sky_cube.dds");
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// The size and the RGB pixels of an image.
type Pixels = (Vec2<usize>, Vec<[f32; 3]>);

fn read_exr(path: &Path) -> Result<Pixels, Box<dyn Error>> {
    let image = exr::prelude::read()
        .no_deep_data()
        .largest_resolution_level()
        .rgba_channels(
            |resolution, _| (resolution, vec![[0.0_f32; 3]; resolution.area()]),
            |(resolution, pixels): &mut Pixels,
             position: Vec2<usize>,
             (r, g, b, _): (f32, f32, f32, f32)| {
                if let Some(slot) = pixels.get_mut(position.y() * resolution.width() + position.x())
                {
                    *slot = [r, g, b];
                }
            },
        )
        .first_valid_layer()
        .all_attributes()
        .from_file(path)?;
    Ok(image.layer_data.channel_data.pixels)
}

fn assert_close(actual: Option<&[f32; 3]>, expected: f32, what: &str) {
    assert!(actual.is_some(), "{what}: no such texel");
    for value in actual.into_iter().flatten() {
        assert!(
            (value - expected).abs() <= expected.abs() * 1e-3,
            "{what}: {value} is {expected}"
        );
    }
}

#[test]
fn a_bc6h_texture_is_written_as_exr_beside_the_requested_png() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = write_block(dir.path(), 95, mode_11(1023, 0))?;

    p4kx()?
        .args(["texture", "png"])
        .arg(&header)
        .arg("--out")
        .arg(dir.path().join("sky.png"))
        .assert()
        .code(0)
        .stdout(predicate::str::contains("as OpenEXR to"))
        .stdout(predicate::str::contains("sky.exr instead of"))
        .stdout(predicate::str::contains("sky.png"))
        .stdout(predicate::str::contains("BC6H_UF16 is a high-range format"));

    assert!(!dir.path().join("sky.png").exists(), "no PNG is written");
    let (size, pixels) = read_exr(&dir.path().join("sky.exr"))?;
    assert_eq!((size.width(), size.height()), (4, 4), "the size");
    assert_close(pixels.first(), 65504.0, "texel 0 keeps the largest half");
    assert_close(pixels.get(1), 0.765_625, "texel 1 at weight 34 of 64");
    assert_close(pixels.get(15), 0.0, "texel 15 is the second endpoint");
    Ok(())
}

#[test]
fn a_signed_bc6h_texture_keeps_negative_values() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = write_block(dir.path(), 96, mode_11(511, 512))?;
    let out = dir.path().join("sky.exr");

    p4kx()?
        .args(["texture", "png"])
        .arg(&header)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);

    let (_, pixels) = read_exr(&out)?;
    assert_close(pixels.first(), 65504.0, "the largest half");
    assert_close(pixels.get(15), -65504.0, "the smallest half");
    Ok(())
}

#[test]
fn tone_map_forces_an_8_bit_png() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = write_block(dir.path(), 95, mode_11(1023, 0))?;
    let out = dir.path().join("sky.png");

    p4kx()?
        .args(["texture", "png", "--tone-map"])
        .arg(&header)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("tone-mapped to 8 bits"));

    let mut reader = png::Decoder::new(std::io::BufReader::new(File::open(&out)?)).read_info()?;
    assert!(
        reader.info().srgb.is_some(),
        "tone-mapped samples are tagged sRGB"
    );
    let mut pixels = vec![0; reader.output_buffer_size().unwrap_or_default()];
    let info = reader.next_frame(&mut pixels)?;
    assert_eq!(info.bit_depth, png::BitDepth::Eight, "8 bits per channel");
    let texels = pixels.as_chunks::<4>().0;
    assert_eq!(texels.first(), Some(&[255; 4]), "65504 maps to white");
    assert_eq!(
        texels.get(1),
        Some(&[176, 176, 176, 255]),
        "0.765625 maps to 176"
    );
    assert_eq!(texels.get(15), Some(&[0, 0, 0, 255]), "zero stays black");
    assert!(!dir.path().join("sky.exr").exists(), "no EXR is written");
    Ok(())
}

#[test]
fn json_names_the_written_format() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = write_block(dir.path(), 95, mode_11(1023, 0))?;

    let output = p4kx()?
        .args(["--json", "texture", "png"])
        .arg(&header)
        .arg("--out")
        .arg(dir.path().join("sky.png"))
        .output()?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["format"], "exr", "{json}");
    assert!(
        json["output"]
            .as_str()
            .is_some_and(|path| path.ends_with("sky.png")),
        "{json}"
    );
    assert!(
        json["files"][0]
            .as_str()
            .is_some_and(|path| path.ends_with("sky.exr")),
        "{json}"
    );
    Ok(())
}

#[test]
fn tone_map_on_a_texture_without_high_range_notes_that_it_has_no_effect() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = SplitDds::new(TestTextureFormat::LegacyDxt1, 8, 8)
        .mips(1)
        .write_files(dir.path(), "panel.dds")?;
    let out = dir.path().join("panel.png");

    p4kx()?
        .args(["texture", "png", "--tone-map"])
        .arg(&header)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0)
        .stderr(predicate::str::contains(
            "--tone-map has no effect: BC1_UNORM is not a high-range format",
        ))
        .stdout(predicate::str::contains("panel.png"));

    assert!(out.exists(), "the PNG is written as usual");
    Ok(())
}

#[test]
fn a_bc6h_cubemap_is_written_as_an_exr_cross_on_request() -> TestResult {
    let dir = tempfile::tempdir()?;
    let format = TestTextureFormat::Dx10Block {
        dxgi_format: 95,
        block_bytes: 16,
    };
    let header = SplitDds::new(format, 8, 8)
        .cubemap()
        .split(2)
        .write_files(dir.path(), "sky_cube.dds")?;

    p4kx()?
        .args(["texture", "png", "--cross"])
        .arg(&header)
        .arg("--out")
        .arg(dir.path().join("sky.png"))
        .assert()
        .code(0)
        .stdout(predicate::str::contains("sky.exr instead of"));

    let (size, pixels) = read_exr(&dir.path().join("sky.exr"))?;
    assert_eq!(
        (size.width(), size.height()),
        (32, 24),
        "a cross of 4 × 3 faces"
    );
    assert!(
        pixels
            .iter()
            .flatten()
            .all(|value| value.is_finite() && *value >= 0.0),
        "unsigned values"
    );
    Ok(())
}
