// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_extraction::model::MaterialSourceLimits;
use p4kx_media::Limits;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, TestMtl, TestMtlMaterial};

use super::{TestResult, verify};

#[test]
fn material_census_reconciles_distinct_root_and_nested_libraries_and_failed_decodes() -> TestResult
{
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("fixture.p4k");
    let source =
        TestMtl::multi(&[TestMtlMaterial::new("retained").attribute("Glow", "0")]).cryxml();
    let nested = TestContainer::new()
        .entry(TestEntry::stored("same.mtl", source.as_slice()))
        .entry(TestEntry::stored("failed.mtl", b"<NotAMaterial/>"))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("same.mtl", source))
        .entry(TestEntry::stored(
            "loose.MTL",
            br#"<Material Custom="source"><Unknown/></Material>"#,
        ))
        .entry(TestEntry::stored("inner.socpak", nested))
        .write_to(&input)?;
    let counts = verify(&Archive::open(input)?, Limits::default())?;
    assert_eq!(
        counts.selected, 4,
        "both container and root libraries are selected"
    );
    assert_eq!(counts.nested, 2, "the original nested occurrences survive");
    assert_eq!(
        counts.complete, 3,
        "valid text and CryXml libraries reconcile"
    );
    assert_eq!(
        counts.failed, 1,
        "raw evidence survives the diagnosed invalid root"
    );
    Ok(())
}

#[test]
fn material_census_reconciles_repacked_batches_with_original_ordinals() -> TestResult {
    const MATERIALS: usize = 24;
    const MAX_RECORD_BYTES: u64 = 32 << 10;
    const UNBATCHED_RECORDS: u64 = 5;
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("batches.p4k");
    let materials: Vec<_> = (0..MATERIALS)
        .map(|index| {
            TestMtlMaterial::new(index.to_string())
                .attribute("Unknown", "x".repeat(256))
                .layer_with(&[("Path", "missing.mtl"), ("Submtl", "Authored")])
                .texture_with_mod("Diffuse", "synthetic.dds", &[("Amount", "3")])
        })
        .collect();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "repacked.mtl",
            TestMtl::multi(&materials).text_xml(),
        ))
        .write_to(&input)?;
    let mut limits = Limits::default();
    limits.record_bytes = MAX_RECORD_BYTES;
    let counts = verify(&Archive::open(input)?, limits)?;
    assert_eq!(
        counts.selected, 1,
        "one physical source contributes the rows"
    );
    assert_eq!(counts.complete, 1, "the source is fully retained");
    assert_eq!(
        counts.materials, MATERIALS,
        "every original material survives"
    );
    assert!(
        counts.records > UNBATCHED_RECORDS,
        "at least one batch family has nonzero starting ordinals"
    );
    Ok(())
}
#[test]
fn material_census_reconciles_raw_bytes_outside_the_parser_admission() -> TestResult {
    let directory = tempfile::tempdir()?;
    let input = directory.path().join("oversized.p4k");
    let bound = MaterialSourceLimits::default().source_bytes;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("oversized.mtl", vec![b' '; bound + 1]))
        .write_to(&input)?;
    let counts = verify(&Archive::open(input)?, Limits::default())?;
    assert_eq!(counts.selected, 1, "the raw source was accounted for");
    assert_eq!(counts.failed, 1, "the structured limit stays explicit");
    assert_eq!(counts.records, 1, "the source has one diagnostic row");
    Ok(())
}
