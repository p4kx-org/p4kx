// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_blender::{Manifest, PackageInfo, PackageWriteContext, write_package_with_context};
use p4kx_extraction::{Archive, ArchiveWriteProtection, CancelToken, FieldPath, OpenOptions};
use p4kx_testkit::{SplitDds, TestEntry, TestTextureFormat, layered_entries, test_key};

use crate::support::{TestResult, assemble, glb_options};

#[test]
fn package_texture_keeps_plain_dds_words_separate_from_used_encrypted_alpha() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let retained = std::env::var_os("P4KX_SYNTHETIC_TEXTURE_PACKAGE").map(PathBuf::from);
    let folder = retained.as_deref().unwrap_or_else(|| temporary.path());
    fs::create_dir_all(folder)?;
    let path = "Data\\Objects\\Test\\layers\\textures\\base_ddna.dds";
    let input = archive(folder, path)?;
    let manifest = write(&input, folder)?;
    let receipt = manifest
        .textures
        .iter()
        .find(|texture| texture.source.ends_with("base_ddna.dds"))
        .and_then(|texture| texture.source_metadata.as_deref())
        .ok_or("same-read DDS metadata")?;
    assert_eq!(receipt.mip_level, 0, "actual requested mip");
    let width = receipt
        .field_origins
        .get(&FieldPath::parse("/dds/normalized/width")?)
        .ok_or("width origin")?;
    assert_eq!(
        width.contains_encrypted_source(),
        Some(false),
        "plain color header supplies width"
    );
    let transfer = receipt.dds.transfer.ok_or("transfer declarations")?;
    assert_eq!(
        transfer.effective,
        p4kx_extraction::texture_preview::DdsTransfer::Unknown,
        "unmarked legacy source"
    );
    let origin = receipt
        .field_origins
        .get(&FieldPath::parse("/dds/transfer/effective")?)
        .ok_or("transfer origin")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(false),
        "alpha does not affect colour transfer"
    );
    let image = receipt
        .field_origins
        .get(&FieldPath::parse("/image")?)
        .ok_or("image origin")?;
    assert_eq!(
        image.contains_encrypted_source(),
        Some(true),
        "actual split alpha supplies PNG pixels"
    );
    assert!(
        image.source_set_complete(),
        "complete color and alpha contributors"
    );
    Ok(())
}

fn archive(folder: &Path, path: &str) -> Result<Archive, Box<dyn Error>> {
    let normal = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
        .alpha(TestTextureFormat::R8)
        .split(1);
    let alpha = normal.alpha_tail().ok_or("alpha fixture")?;
    let mut archive = p4kx_testkit::ArchiveBuilder::new();
    let part = format!("{path}.1");
    for entry in layered_entries() {
        if entry.path() != path && entry.path() != part {
            archive = archive.entry(entry);
        }
    }
    let output = folder.join("Data.p4k");
    archive
        .entry(TestEntry::stored(path, normal.header_bytes()))
        .entry(TestEntry::stored(part, normal.part(1)))
        .entry(TestEntry::stored(format!("{path}.a"), alpha.header_bytes()))
        .entry(TestEntry::stored(format!("{path}.1a"), alpha.part(1)).encrypted())
        .write_to(&output)?;
    Ok(Archive::open_with(
        output,
        OpenOptions::new().key(Some(test_key())),
    )?)
}
fn write(archive: &Archive, output: &Path) -> Result<Manifest, Box<dyn Error>> {
    let assembly = assemble(archive, "TEST_Layered")?;
    let models = p4kx_assembly::ArchiveModels::new(archive);
    let loader = models.textures();
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let context =
        PackageWriteContext::new(&protection, cancel.flag()).with_archive_texture_source(&loader);
    let package = output.join("ship");
    write_package_with_context(
        &assembly,
        &models,
        &loader,
        &glb_options(0),
        &PackageInfo::default(),
        &package,
        &context,
    )?;
    let bytes = fs::read(package.join("scene.json"))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let schema = p4kx_blender::manifest_schema();
    jsonschema::validator_for(&schema)?
        .validate(&value)
        .map_err(|error| error.to_string())?;
    Ok(serde_json::from_slice(&bytes)?)
}
