// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx texture png` end to end: PNG pixel snapshots per format, mip choice and failures.

use std::error::Error;
use std::fmt::Write as _;
use std::fs::File;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestTextureFormat};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn Error>>;

#[path = "texture_png_transfer.rs"]
mod transfer;

const HEADER: &str = "Data\\Textures\\panel_diff.dds";

fn p4kx() -> Result<Command, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?)
}

fn write_archive(folder: &Path, dds: &SplitDds) -> Result<PathBuf, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    dds.entries(HEADER)
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

/// Reads an 8-bit RGBA PNG, returning its width, height and pixels.
fn read_rgba(path: &Path) -> Result<(u32, u32, Vec<u8>), Box<dyn Error>> {
    let mut reader = png::Decoder::new(std::io::BufReader::new(File::open(path)?)).read_info()?;
    let mut pixels = vec![0; reader.output_buffer_size().unwrap_or_default()];
    let info = reader.next_frame(&mut pixels)?;
    assert_eq!(info.color_type, png::ColorType::Rgba, "RGBA");
    assert_eq!(info.bit_depth, png::BitDepth::Eight, "8 bits per channel");
    pixels.truncate(info.buffer_size());
    Ok((info.width, info.height, pixels))
}

/// Reads an 8-bit RGBA PNG and renders its pixels as one line of `RRGGBBAA` values per row.
fn pixel_snapshot(path: &Path) -> Result<String, Box<dyn Error>> {
    let (width, height, pixels) = read_rgba(path)?;
    let mut text = format!("{width} x {height}\n");
    let row_len = width as usize * 4;
    for row in pixels.chunks_exact(row_len) {
        let line: Vec<String> = row
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| {
                pixel
                    .iter()
                    .fold(String::new(), |hex, byte| hex + &format!("{byte:02x}"))
            })
            .collect();
        writeln!(text, "{}", line.join(" "))?;
    }
    Ok(text)
}

const fn block_format(dxgi_format: u32, block_bytes: u32) -> TestTextureFormat {
    TestTextureFormat::Dx10Block {
        dxgi_format,
        block_bytes,
    }
}

/// Writes a texture of one mip with `blocks` as its data to `folder`, returning its path.
fn write_blocks(
    folder: &Path,
    format: TestTextureFormat,
    (width, height): (u32, u32),
    blocks: &[u8],
) -> Result<PathBuf, Box<dyn Error>> {
    let dds = SplitDds::new(format, width, height).mips(1);
    let mut bytes = dds.header_bytes();
    bytes.truncate(dds.header_len());
    bytes.extend_from_slice(blocks);
    let path = folder.join("blocks.dds");
    std::fs::write(&path, bytes)?;
    Ok(path)
}

fn export_file(header: &Path) -> Result<String, Box<dyn Error>> {
    let out = header.with_extension("png");
    p4kx()?
        .args(["texture", "png"])
        .arg(header)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);
    pixel_snapshot(&out)
}

fn export_entry(dds: &SplitDds, mip: Option<&str>) -> Result<String, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), dds)?;
    let out = dir.path().join("panel_diff.png");
    let mut command = p4kx()?;
    command
        .args(["texture", "png"])
        .arg(&archive)
        .arg(HEADER)
        .arg("--out")
        .arg(&out);
    if let Some(level) = mip {
        command.args(["--mip", level]);
    }
    command.assert().code(0);
    pixel_snapshot(&out)
}

#[test]
fn every_supported_format_exports_its_largest_mip() -> TestResult {
    let formats = [
        ("bc1", TestTextureFormat::LegacyDxt1),
        (
            "bc2",
            TestTextureFormat::Dx10Block {
                dxgi_format: 74,
                block_bytes: 16,
            },
        ),
        ("bc3", TestTextureFormat::LegacyDxt5),
        ("b8g8r8a8", TestTextureFormat::LegacyBgra8),
        ("b8g8r8", TestTextureFormat::LegacyBgr8),
        ("l8a8", TestTextureFormat::LegacyL8A8),
        (
            "b8g8r8x8",
            TestTextureFormat::Dx10Pixel {
                dxgi_format: 88,
                pixel_bytes: 4,
            },
        ),
        (
            "r8",
            TestTextureFormat::Dx10Pixel {
                dxgi_format: 61,
                pixel_bytes: 1,
            },
        ),
        (
            "r16",
            TestTextureFormat::Dx10Pixel {
                dxgi_format: 56,
                pixel_bytes: 2,
            },
        ),
        ("bc4", block_format(80, 8)),
        ("bc4_snorm", block_format(81, 8)),
        ("bc5", block_format(83, 16)),
        ("bc5_snorm", block_format(84, 16)),
        ("bc7", TestTextureFormat::BC7_SRGB),
    ];
    for (name, format) in formats {
        let snapshot = export_entry(&SplitDds::new(format, 8, 8).split(2), None)?;
        insta::assert_snapshot!(format!("largest_mip_{name}"), snapshot);
    }
    Ok(())
}

/// One BC7 block per mode 0 to 7 and one of the reserved mode, their other bits pseudo-random.
fn bc7_blocks_of_every_mode() -> Vec<u8> {
    let mut state = 0x0007_8B07_u64;
    let mut next = move || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        u128::from(mixed ^ (mixed >> 31))
    };
    (0..=8_u32)
        .flat_map(|mode| {
            let random = (next() << 64) | next();
            let mode_bit = if mode < 8 { 1 << mode } else { 0 };
            ((random & !((1 << (mode + 1)) - 1)) | mode_bit).to_le_bytes()
        })
        .collect()
}

#[test]
fn bc7_blocks_of_every_mode_export_to_known_pixels() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = write_blocks(
        dir.path(),
        block_format(98, 16),
        (36, 4),
        &bc7_blocks_of_every_mode(),
    )?;

    insta::assert_snapshot!(export_file(&header)?);
    Ok(())
}

#[test]
fn a_bc5_normal_map_exports_unit_normals_in_rgb() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = SplitDds::new(block_format(83, 16), 32, 32)
        .split(2)
        .write_files(dir.path(), "hull_ddna.dds")?;
    export_file(&header)?;

    let (width, height, pixels) = read_rgba(&header.with_extension("png"))?;

    assert_eq!((width, height), (32, 32), "the largest mip");
    let unit = |value: u8| f32::from(value) / 127.5 - 1.0;
    for pixel in pixels.as_chunks::<4>().0 {
        let [x, y, z, alpha] = pixel.map(unit);
        let planar = x.mul_add(x, y * y);
        let length = z.mul_add(z, planar);
        assert!(
            (planar > 1.0 && pixel[2] == 128) || (length - 1.0).abs() < 0.03,
            "{pixel:?} is a unit normal, Z 0 when X and Y are already longer"
        );
        assert!((alpha - 1.0).abs() < f32::EPSILON, "{pixel:?} is opaque");
    }
    Ok(())
}

#[test]
fn mip_picks_a_smaller_level_from_a_file_on_disk() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = SplitDds::new(TestTextureFormat::LegacyDxt5, 16, 8)
        .split(2)
        .write_files(dir.path(), "panel_diff.dds")?;
    let out = dir.path().join("small.png");

    p4kx()?
        .args(["texture", "png"])
        .arg(&header)
        .arg("--out")
        .arg(&out)
        .args(["--mip", "2"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("wrote mip 2"))
        .stdout(predicate::str::contains("4 x 2"));

    insta::assert_snapshot!(pixel_snapshot(&out)?);
    Ok(())
}

#[test]
fn the_mip_from_a_part_matches_the_mip_from_a_merged_file() -> TestResult {
    let dds = SplitDds::new(TestTextureFormat::LegacyDxt1, 16, 16).split(3);
    let unsplit = SplitDds::new(TestTextureFormat::LegacyDxt1, 16, 16);

    for level in ["0", "1", "3"] {
        assert_eq!(
            export_entry(&dds, Some(level))?,
            export_entry(&unsplit, Some(level))?,
            "mip {level}"
        );
    }
    Ok(())
}

#[test]
fn an_unsupported_format_is_named_and_fails_with_code_1() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(
        dir.path(),
        &SplitDds::new(
            TestTextureFormat::Dx10Pixel {
                dxgi_format: 32,
                pixel_bytes: 4,
            },
            16,
            16,
        )
        .split(1),
    )?;
    let out = dir.path().join("panel_diff.png");

    p4kx()?
        .args(["texture", "png"])
        .arg(&archive)
        .arg(HEADER)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "decoding the texture format R8G8B8A8_SINT is not supported",
        ));

    assert!(!out.exists(), "nothing is written");
    Ok(())
}

#[test]
fn a_mip_the_texture_does_not_have_fails_with_code_2() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = SplitDds::new(TestTextureFormat::LegacyDxt1, 8, 8)
        .write_files(dir.path(), "panel_diff.dds")?;

    p4kx()?
        .args(["texture", "png"])
        .arg(&header)
        .arg("--out")
        .arg(dir.path().join("panel_diff.png"))
        .args(["--mip", "4"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("mip 4 does not exist"));
    Ok(())
}
