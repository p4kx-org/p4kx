// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use p4kx_assembly::ArchiveModels;
use p4kx_blender::{
    AlphaMeaning, DdsTextureFallback, Manifest, PackageInfo, PackageTextureContainer,
    PackageWriteContext, Texture, TextureFormat, write_package_with_context,
};
use p4kx_extraction::texture_codec::{DdsAlphaAssociation, DdsAlphaTransform};
use p4kx_extraction::{Archive, ArchiveWriteProtection, CancelToken, FieldPath};
use p4kx_testkit::{SplitDds, TestEntry, TestTextureFormat, layered_entries, test_key};

use crate::support::{Fixture, TestResult, assemble, fixture, glb_options};

const ALPHA_RGBA: [u8; 4] = [64, 32, 128, 128];
const PREMULTIPLIED_PATH: &str = r"Data\Objects\Test\layers\textures\base_diff.dds";
const COVERAGE_PATH: &str = r"Data\Objects\Test\layers\textures\wear_diff.dds";
const SPLIT_PATH: &str = r"Data\Objects\Test\layers\textures\wear_spec.dds";
const SPLIT_TAIL: &str = r"Data\Objects\Test\layers\textures\wear_spec.dds.a";

#[test]
fn premultiplied_opacity_uses_straight_png_and_bc1_uses_original_dds() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let retained = std::env::var_os("P4KX_SYNTHETIC_ALPHA_PACKAGE").map(PathBuf::from);
    let folder = retained.as_deref().unwrap_or_else(|| temporary.path());
    let (_fixture, archive) = archive()?;
    let assembly = assemble(&archive, "TEST_Layered")?;
    let models = ArchiveModels::new(&archive);
    let loader = models.textures();
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let context = PackageWriteContext::new(&protection, cancel.flag())
        .with_archive_texture_source(&loader)
        .with_texture_container(PackageTextureContainer::BlenderDds);
    let output = folder.join("mixed-alpha");
    write_package_with_context(
        &assembly,
        &models,
        &loader,
        &glb_options(0),
        &PackageInfo::default(),
        &output,
        &context,
    )?;
    let manifest: Manifest = serde_json::from_slice(&fs::read(output.join("scene.json"))?)?;
    let find = |suffix: &str| {
        manifest
            .textures
            .iter()
            .find(|texture| texture.source.ends_with(suffix))
    };
    verify_png(find("base_diff.dds").ok_or("opacity PNG")?, &output)?;
    verify_dds(find("wear_diff.dds").ok_or("original BC1")?)?;
    verify_split(find("wear_spec.dds").ok_or("split opacity PNG")?, &output)?;
    Ok(())
}

fn verify_png(row: &Texture, output: &Path) -> TestResult {
    assert_eq!(
        row.format,
        TextureFormat::Png,
        "associated opacity needs straight PNG"
    );
    assert_eq!(
        row.dds_fallback,
        Some(DdsTextureFallback::PremultipliedAlpha),
        "explicit capability refusal"
    );
    assert_eq!(
        row.alpha,
        AlphaMeaning::Opacity,
        "PNG sampling treats alpha as opacity"
    );
    let source = row
        .source_metadata
        .as_deref()
        .ok_or("PNG source metadata")?;
    assert_eq!(
        source.dds.alpha_association,
        Some(DdsAlphaAssociation::Premultiplied),
        "authored association"
    );
    assert_eq!(
        source.alpha_transform,
        Some(DdsAlphaTransform::UnassociateLinear),
        "actual output transformation"
    );
    let origin = source
        .field_origins
        .get(&FieldPath::parse("/alpha_transform")?)
        .ok_or("transformation origin")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the exact header was encrypted"
    );
    let pixels = png_pixels(row, output)?;
    let samples = pixels.as_chunks::<4>().0;
    assert_eq!(
        samples.first(),
        Some(&[0; 4]),
        "transparent pixels deterministically become black"
    );
    assert_eq!(
        samples.get(1),
        Some(&[128, 64, 255, 128]),
        "linear straight opacity"
    );
    Ok(())
}

fn verify_split(row: &Texture, output: &Path) -> TestResult {
    assert_eq!(row.format, TextureFormat::Png, "split alpha requires PNG");
    assert_eq!(
        row.dds_fallback,
        Some(DdsTextureFallback::SplitAlpha),
        "split input cannot be emitted as one original surface"
    );
    let pixels = png_pixels(row, output)?;
    let samples = pixels.as_chunks::<4>().0;
    assert_eq!(
        samples.first(),
        Some(&[0, 0, 0, 64]),
        "zero embedded opacity"
    );
    assert_eq!(
        samples.get(1),
        Some(&[128, 64, 255, 64]),
        "RGB uses embedded opacity 128, output opacity comes from the tail"
    );
    Ok(())
}

fn png_pixels(row: &Texture, output: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut png =
        png::Decoder::new(BufReader::new(File::open(output.join(&row.file))?)).read_info()?;
    let mut pixels = vec![0; png.output_buffer_size().ok_or("buffer length")?];
    png.next_frame(&mut pixels)?;
    Ok(pixels)
}

fn verify_dds(row: &Texture) -> TestResult {
    assert_eq!(row.format, TextureFormat::Dds, "BC1 is supported");
    assert_eq!(
        row.alpha,
        AlphaMeaning::Opacity,
        "the selected BC1 actually contains transparency"
    );
    let asset = row.dds_asset.as_deref().ok_or("DDS source metadata")?;
    let coverage = asset.alpha_coverage.ok_or("measured BC1 alpha")?;
    assert_eq!(
        (coverage.zero, coverage.partial, coverage.full),
        (1, 0, 5),
        "padding is excluded"
    );
    for suffix in ["zero", "partial", "full"] {
        let field = FieldPath::parse(&format!("/alpha_coverage/{suffix}"))?;
        let origin = asset
            .field_origins
            .get(&field)
            .ok_or("selected payload origin")?;
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "coverage uses encrypted selected blocks"
        );
        assert!(
            origin.source_set_complete(),
            "physical header and mip are exact"
        );
    }
    Ok(())
}

fn archive() -> Result<(Fixture, Archive), Box<dyn Error>> {
    let rgba = SplitDds::new(
        TestTextureFormat::Dx10Pixel {
            dxgi_format: 28,
            pixel_bytes: 4,
        },
        4,
        4,
    )
    .mips(1)
    .alpha(TestTextureFormat::R8);
    let tail = rgba.alpha_tail().ok_or("separate opacity")?;
    let mut tail_bytes = tail.header_bytes();
    tail_bytes
        .get_mut(tail.header_len()..)
        .ok_or("tail pixels")?
        .fill(64);
    let mut rgba_bytes = rgba.header_bytes();
    rgba_bytes
        .get_mut(144..148)
        .ok_or("miscFlags2")?
        .copy_from_slice(&2_u32.to_le_bytes());
    let payload = rgba_bytes.get_mut(rgba.header_len()..).ok_or("RGBA mip")?;
    for pixel in payload.as_chunks_mut::<4>().0 {
        *pixel = ALPHA_RGBA;
    }
    payload
        .get_mut(..4)
        .ok_or("first pixel")?
        .copy_from_slice(&[64, 32, 128, 0]);
    let bc1 = SplitDds::new(TestTextureFormat::LegacyDxt1, 3, 2)
        .mips(1)
        .split(1);
    let part_path = format!("{COVERAGE_PATH}.1");
    let first = [0_u32.to_le_bytes(), 0xC3_u32.to_le_bytes()].concat();
    let entries = layered_entries()
        .into_iter()
        .filter(|entry| {
            entry.path() != PREMULTIPLIED_PATH
                && !entry.path().starts_with(COVERAGE_PATH)
                && !entry.path().starts_with(SPLIT_PATH)
        })
        .chain([
            TestEntry::stored(PREMULTIPLIED_PATH, rgba_bytes.clone()).encrypted(),
            TestEntry::stored(SPLIT_PATH, rgba_bytes),
            TestEntry::stored(SPLIT_TAIL, tail_bytes).encrypted(),
            TestEntry::stored(COVERAGE_PATH, bc1.header_bytes()),
            TestEntry::stored(part_path, first).encrypted(),
        ])
        .collect();
    let fixture = fixture(entries)?;
    let archive = Archive::open_with_key(&fixture.path, Some(test_key()))?;
    Ok((fixture, archive))
}
