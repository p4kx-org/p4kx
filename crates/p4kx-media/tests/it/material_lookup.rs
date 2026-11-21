// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Filter;
use p4kx_media::{MaterialProvider, Status};
use p4kx_testkit::TestEntry;
use serde_json::json;

use super::material_lookup_support::{field, reference, status, target_index};
use super::material_support::{self as support, TestResult};

const SOURCE: &[u8] = br#"<Material DefaultPalette="record/palette"><MatLayers><Layer Path="missing.mtl" Submtl="007"/></MatLayers><Textures><Texture Map="TexSlot1" File="Textures/missing.tif"/><Texture Map="TexSlot2" File="$EngineTexture"/></Textures></Material>"#;

#[test]
fn complete_inventory_proves_missing_file_targets_without_guessing_runtime_bindings() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let archive = support::archive(folder.path(), [TestEntry::stored("Data/a.mtl", SOURCE)])?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &support::registry(MaterialProvider::default())?,
    )?)?;
    let records = support::records(&output, &index)?;
    let record = records.first().ok_or("library")?;
    assert_eq!(
        record.status,
        Status::Complete,
        "complete inventory proves missing file targets without guessing runtime bindings"
    );
    status(reference(record, 0)?, "unavailable")?;
    status(reference(record, 1)?, "missing")?;
    status(reference(record, 2)?, "missing")?;
    status(reference(record, 3)?, "unavailable")?;
    assert_eq!(
        field(&record.data, "/payload/references/1/submaterial")?,
        &json!("007"),
        "complete inventory proves missing file targets without guessing runtime bindings"
    );
    assert_eq!(
        field(&record.data, "/payload/default_palette")?,
        &json!("record/palette"),
        "complete inventory proves missing file targets without guessing runtime bindings"
    );
    for ordinal in [0, 3] {
        assert_eq!(
            field(reference(record, ordinal)?, "/resolution/reason/type")?,
            &json!("target-not-read"),
            "complete inventory proves missing file targets without guessing runtime bindings"
        );
    }
    Ok(())
}

#[test]
fn duplicate_layer_and_texture_paths_preserve_every_physical_candidate() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = br#"<Material><MatLayers><Layer Path="Data/B.MTL"/></MatLayers><Textures><Texture Map="TexSlot1" File="Textures/MAP.tif"/></Textures></Material>"#;
    let archive = support::archive(
        folder.path(),
        [
            TestEntry::stored("Data/a.mtl", source),
            TestEntry::stored("Data/b.mtl", b"<Material/>"),
            TestEntry::stored("data/B.MTL", b"<Material Name='other'/>"),
            TestEntry::stored("Data/Textures/map.dds", b"raw-one"),
            TestEntry::stored("data/textures/MAP.DDS", b"raw-two"),
        ],
    )?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &support::registry(MaterialProvider::default())?,
    )?)?;
    let records = support::records(&output, &index)?;
    let record = records.first().ok_or("source library")?;
    for (ordinal, expected) in [(0, [1, 2]), (1, [3, 4])] {
        let reference = reference(record, ordinal)?;
        status(reference, "ambiguous")?;
        let candidates = field(reference, "/resolution/candidates")?
            .as_array()
            .ok_or("candidates")?;
        let actual: Result<Vec<_>, Box<dyn std::error::Error>> = candidates
            .iter()
            .map(|candidate| {
                field(candidate, "/target/source/entry/entry_index")?
                    .as_u64()
                    .ok_or_else(|| "candidate index".into())
            })
            .collect();
        assert_eq!(
            actual?, expected,
            "duplicate layer and texture paths preserve every physical candidate"
        );
    }
    assert_eq!(
        records.len(),
        3,
        "duplicate libraries are parsed once per physical source"
    );
    Ok(())
}

#[test]
fn an_existing_filtered_target_is_not_missing_and_requires_no_target_read() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = br#"<Material><MatLayers><Layer Path="b" Submtl="Body"/></MatLayers></Material>"#;
    let archive = support::archive(
        folder.path(),
        [
            TestEntry::stored("Data/a.mtl", source),
            TestEntry::stored("Data/b.mtl", b"unreadable but filtered"),
        ],
    )?;
    let output = folder.path().join("out");
    let mut options = support::options();
    options.include = Some(Filter::parse("*a.mtl")?);
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &options,
        &support::registry(MaterialProvider::default())?,
    )?)?;
    let records = support::records(&output, &index)?;
    assert_eq!(
        records.len(),
        1,
        "an existing filtered target is not missing and requires no target read"
    );
    let reference = reference(records.first().ok_or("source")?, 0)?;
    status(reference, "not-selected")?;
    assert_eq!(
        target_index(reference)?,
        1,
        "an existing filtered target is not missing and requires no target read"
    );
    assert_eq!(
        field(reference, "/resolution/reason")?,
        &json!("filter"),
        "an existing filtered target is not missing and requires no target read"
    );
    assert_eq!(
        index.source_entries.skipped, 1,
        "an existing filtered target is not missing and requires no target read"
    );
    Ok(())
}

#[test]
fn invalid_identifiers_are_retained_without_a_fabricated_missing_target() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = br#"<Material><MatLayers><Layer Path="../outside"/></MatLayers><Textures><Texture Map="TexSlot1" File="C:\outside.tif"/></Textures></Material>"#;
    let archive = support::archive(folder.path(), [TestEntry::stored("Data/a.mtl", source)])?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &support::registry(MaterialProvider::default())?,
    )?)?;
    let records = support::records(&output, &index)?;
    for ordinal in [0, 1] {
        let reference = reference(records.first().ok_or("source")?, ordinal)?;
        status(reference, "unavailable")?;
        assert_eq!(
            field(reference, "/resolution/reason/type")?,
            &json!("invalid-identifier"),
            "invalid identifiers are retained without a fabricated missing target"
        );
    }
    Ok(())
}
