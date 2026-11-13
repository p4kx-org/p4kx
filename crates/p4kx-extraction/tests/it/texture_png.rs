// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Decoding split DDS textures and exporting a mip as PNG.

use std::error::Error;
use std::fs::File;
use std::path::Path;

use p4kx_extraction::texture_codec::{DdsPngEncoding, DdsTransfer};
use p4kx_extraction::{Archive, ConversionError, PngReport, Texture, TextureError, TextureFormat};
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestTextureFormat};

type TestResult = Result<(), Box<dyn Error>>;

const HEADER: &str = "Data\\Textures\\crate_diff.dds";

fn texture() -> SplitDds {
    SplitDds::new(TestTextureFormat::LegacyDxt5, 32, 16).split(2)
}

fn write_archive(folder: &Path, dds: &SplitDds) -> Result<Archive, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    dds.entries(HEADER)
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(Archive::open_with_key(
        &path,
        Some(p4kx_testkit::test_key()),
    )?)
}

fn read_png(path: &Path) -> Result<(u32, u32, Vec<u8>), Box<dyn Error>> {
    let mut reader = png::Decoder::new(std::io::BufReader::new(File::open(path)?)).read_info()?;
    let mut pixels = vec![0; reader.output_buffer_size().unwrap_or_default()];
    let info = reader.next_frame(&mut pixels)?;
    Ok((info.width, info.height, pixels))
}

#[test]
fn the_largest_mip_of_an_entry_is_exported_as_png() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), &texture())?;
    let texture = Texture::from_archive(&archive, HEADER)?;
    let out = dir.path().join("crate_diff.png");

    let report = texture.png_to_file(&out, 0)?;

    assert_eq!(
        report,
        PngReport {
            level: 0,
            width: 32,
            height: 16,
            files: vec![out.clone()],
            alpha_tail: false,
            encoding: DdsPngEncoding::of(texture.header().format(), DdsTransfer::Unknown),
        },
        "the report"
    );
    let (width, height, pixels) = read_png(&out)?;
    assert_eq!((width, height), (32, 16), "the PNG's size");
    assert_eq!(
        pixels,
        texture.decode_mip(0)?.into_pixels(),
        "the decoded pixels"
    );
    Ok(())
}

#[test]
fn a_smaller_mip_of_a_file_is_exported_from_the_header_tail() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = texture().write_files(dir.path(), "crate_diff.dds")?;
    let texture = Texture::from_file(&header)?;
    let out = dir.path().join("small.png");

    let report = texture.png_to_file(&out, 3)?;

    assert_eq!((report.width, report.height), (4, 2), "mip 3 of 32×16");
    assert_eq!(read_png(&out)?.0, 4, "the PNG holds mip 3");
    Ok(())
}

#[test]
fn an_unsupported_format_fails_before_writing_anything() -> TestResult {
    let dir = tempfile::tempdir()?;
    let signed_integer = TestTextureFormat::Dx10Pixel {
        dxgi_format: 32,
        pixel_bytes: 4,
    };
    let dds = SplitDds::new(signed_integer, 16, 16).split(1);
    let header = dds.write_files(dir.path(), "hull.dds")?;
    let texture = Texture::from_file(&header)?;
    let out = dir.path().join("hull.png");

    let result = texture.png_to_file(&out, 0);

    let Err(ConversionError::Texture {
        source: TextureError::UnsupportedDecode(format),
        ..
    }) = result
    else {
        panic!("R8G8B8A8_SINT has no decoder: {result:?}");
    };
    assert_eq!(format.to_string(), "R8G8B8A8_SINT", "the format is named");
    assert!(matches!(format, TextureFormat::Dxgi(_)), "a DXGI format");
    assert!(!out.exists(), "nothing is written");
    Ok(())
}

#[test]
fn a_missing_mip_level_and_an_output_that_is_an_input_are_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = texture().write_files(dir.path(), "crate_diff.dds")?;
    let texture = Texture::from_file(&header)?;

    let beyond = texture.png_to_file(&dir.path().join("x.png"), 6);
    let onto_part = texture.png_to_file(&dir.path().join("crate_diff.dds.1"), 0);

    assert!(
        matches!(
            beyond,
            Err(ConversionError::Texture {
                source: TextureError::NoSuchMip {
                    level: 6,
                    mip_count: 6
                },
                ..
            })
        ),
        "no mip 6: {beyond:?}"
    );
    assert!(
        matches!(onto_part, Err(ConversionError::OutputIsInput { .. })),
        "a part is an input: {onto_part:?}"
    );
    Ok(())
}
