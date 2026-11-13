// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::BufReader;

use p4kx_assembly::ArchiveModels;
use p4kx_blender::{
    AlphaMeaning, DdsTextureFallback, Manifest, PackageInfo, PackageTextureContainer,
    PackageWriteContext, TextureFormat, write_package_with_context,
};
use p4kx_extraction::{Archive, ArchiveWriteProtection, CancelToken, FieldPath};
use p4kx_testkit::{SplitDds, TestEntry, TestTextureFormat, layered_entries, test_key};

use crate::support::{TestResult, assemble, fixture, glb_options};

const NORMAL: &str = r"Data\Objects\Test\layers\textures\base_ddna.dds";

const ALPHA_SAMPLE: u8 = 149;
const BASE_LEVEL_R8_BYTES: usize = 8 * 8;

#[test]
fn fallback_uses_the_shared_last_mip_and_actual_heterogeneous_alpha_pixels() -> TestResult {
    let (_fixture, source) = source()?;
    let assembly = assemble(&source, "TEST_Layered")?;
    let models = ArchiveModels::new(&source);
    let loader = models.textures();
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(source.path(), cancel.flag())?;
    let output = tempfile::tempdir()?;
    let package = output.path().join("blender");
    let context = PackageWriteContext::new(&protection, cancel.flag())
        .with_archive_texture_source(&loader)
        .with_texture_container(PackageTextureContainer::BlenderDds);
    write_package_with_context(
        &assembly,
        &models,
        &loader,
        &glb_options(9),
        &PackageInfo::default(),
        &package,
        &context,
    )?;
    let manifest: Manifest = serde_json::from_slice(&fs::read(package.join("scene.json"))?)?;
    let item = manifest
        .textures
        .iter()
        .find(|item| item.source.ends_with("base_ddna.dds"))
        .ok_or("composed normal row")?;
    assert_eq!(
        item.format,
        TextureFormat::Png,
        "the distinct DDS streams are composed"
    );
    assert_eq!(
        item.dds_fallback,
        Some(DdsTextureFallback::SplitAlpha),
        "why PNG was required"
    );
    assert_eq!(
        item.alpha,
        AlphaMeaning::Smoothness,
        "normal alpha stays a data channel"
    );
    assert_eq!(
        (item.width, item.height),
        (4, 4),
        "alpha has only two levels"
    );
    let metadata = item.source_metadata.as_deref().ok_or("fallback receipt")?;
    assert_eq!(metadata.mip_level, 1, "both source DDS streams use mip one");
    let mip_origin = metadata
        .field_origins
        .get(&FieldPath::parse("/mip_level")?)
        .ok_or("mip origin")?;
    assert_eq!(
        mip_origin.contains_encrypted_source(),
        Some(true),
        "the alpha header limits the mip"
    );
    verify_alpha(&package.join(&item.file))
}

fn verify_alpha(path: &std::path::Path) -> TestResult {
    let reader = BufReader::new(fs::File::open(path)?);
    let mut png = png::Decoder::new(reader).read_info()?;
    let mut bytes = vec![0; png.output_buffer_size().ok_or("image size")?];
    png.next_frame(&mut bytes)?;
    assert!(
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == ALPHA_SAMPLE),
        "alpha pixels come from the actual selected R8 mip"
    );
    Ok(())
}

fn source() -> Result<(crate::support::Fixture, Archive), Box<dyn Error>> {
    let normal = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
        .mips(4)
        .split(1);
    let alpha = SplitDds::new(TestTextureFormat::R8, 8, 8)
        .mips(2)
        .alpha(TestTextureFormat::R8)
        .alpha_tail()
        .ok_or("a short alpha tail")?;
    let mut alpha_data = alpha.header_bytes();
    let start = alpha.header_len();
    let smallest = alpha_data
        .get_mut(start + BASE_LEVEL_R8_BYTES..)
        .ok_or("alpha mip one")?;
    smallest.fill(ALPHA_SAMPLE);
    let part_path = format!("{NORMAL}.1");
    let entries = layered_entries()
        .into_iter()
        .filter(|entry| entry.path() != NORMAL && entry.path() != part_path)
        .chain(normal.entries(NORMAL))
        .chain([TestEntry::stored(format!("{NORMAL}.a"), alpha_data).encrypted()])
        .collect();
    let fixture = fixture(entries)?;
    let source = Archive::open_with_key(&fixture.path, Some(test_key()))?;
    Ok((fixture, source))
}
