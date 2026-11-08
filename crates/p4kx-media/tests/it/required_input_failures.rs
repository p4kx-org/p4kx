// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::sync::{Arc, Mutex};

use p4kx_extraction::{ConflictPolicy, Exclusion, ExtractionError};
use p4kx_media::{ExportOptions, Identity, InputReceipt, MediaError, ProviderRegistry};
use p4kx_testkit::TestEntry;

use super::required_input_support::{self as input, InputProvider};
use super::support::{self, TestResult};

#[test]
fn excluded_header_keeps_its_family_out_of_callbacks_and_retention() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", b"needed"),
        ],
    )?;
    let options = ExportOptions {
        exclude: Exclusion::parse(["*.dds"])?,
        ..ExportOptions::default()
    };
    let output = folder.path().join("media");
    let (index, visits) = input::run(&archive, &output, &options, 1, 1)?;
    assert!(
        visits.is_empty(),
        "excluded family does not become selected through input access"
    );
    assert!(
        input::sources(&output, &index)?.is_empty(),
        "no automatic excluded-family retention"
    );
    assert_eq!(
        index.source_entries.skipped, 2,
        "both physical entries stay unselected"
    );
    Ok(())
}

#[test]
fn source_io_failure_stays_typed_and_terminal_even_in_current_diagnostic_mode() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", b"bad").with_crc32c(1),
        ],
    )?;
    let output = folder.path().join("media");
    let initial = ExportOptions {
        exclude: Exclusion::parse(["*.dds"])?,
        ..ExportOptions::default()
    };
    input::run(&archive, &output, &initial, 1, 1)?;
    let previous = fs::read(output.join("media-index.json"))?;
    for diagnostic in [false, true] {
        let options = ExportOptions {
            exclude: Exclusion::parse(["*.dds.1"])?,
            conflict: ConflictPolicy::Overwrite,
            diagnostic,
            ..ExportOptions::default()
        };
        let providers = ProviderRegistry::try_new(vec![Box::new(InputProvider {
            uses: 1,
            rereads: 1,
            prefix: None,
            probe: false,
            visits: Arc::new(Mutex::new(Vec::new())),
        })])?;
        let failure = p4kx_media::export_with(&archive, &output, &options, &providers)
            .err()
            .ok_or("corrupt required input succeeded")?;
        assert_checksum_source(&failure);
        assert_eq!(
            fs::read(output.join("media-index.json"))?,
            previous,
            "failed read never replaces the old complete index"
        );
    }
    Ok(())
}

fn assert_checksum_source(failure: &MediaError) {
    let mut current: Option<&(dyn Error + 'static)> = Some(failure);
    let mut original_checksum = false;
    let mut physical_part = false;
    while let Some(error) = current {
        original_checksum |= matches!(
            error.downcast_ref::<ExtractionError>(),
            Some(ExtractionError::ChecksumMismatch { .. }),
        );
        physical_part |= matches!(
            error.downcast_ref::<MediaError>(),
            Some(MediaError::Source { entry, .. })
                if matches!(&entry.identity, Identity::Entry { entry, .. } if entry.entry_index == 1),
        );
        current = error.source();
    }
    assert!(
        original_checksum,
        "original checksum failure remains downcastable: {failure:?}"
    );
    assert!(
        physical_part,
        "failure names the proved required physical Entry"
    );
}

#[test]
fn input_receipt_requires_its_explicit_nullable_omission_key() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", b"needed"),
        ],
    )?;
    let output = folder.path().join("media");
    let (index, _) = input::run(&archive, &output, &ExportOptions::default(), 1, 1)?;
    let row = input::row(&output, &index)?;
    let receipt = row.pointer("/inputs/0").ok_or("actual input receipt")?;
    let validator = jsonschema::validator_for(&schemars::schema_for!(InputReceipt).to_value())?;
    assert!(
        validator.is_valid(receipt),
        "actual retained input receipt conforms"
    );
    assert_eq!(
        receipt.get("omission"),
        Some(&serde_json::Value::Null),
        "no omitted nullable key"
    );
    let mut missing = receipt.clone();
    missing
        .as_object_mut()
        .ok_or("receipt object")?
        .remove("omission");
    assert!(
        !validator.is_valid(&missing),
        "missing differs from explicit null"
    );
    Ok(())
}

#[test]
fn a_prefix_read_keeps_the_measured_complete_materialisation_digest() -> TestResult {
    use sha2::{Digest as _, Sha256};

    let folder = tempfile::tempdir()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("owner.dds", b"H"),
            TestEntry::stored("owner.dds.1", b"needed"),
        ],
    )?;
    let output = folder.path().join("media");
    let mut options = ExportOptions {
        exclude: Exclusion::parse(["*.dds.1"])?,
        ..ExportOptions::default()
    };
    options.limits.source_bytes = 8;
    let providers = ProviderRegistry::try_new(vec![Box::new(InputProvider {
        uses: 1,
        rereads: 1,
        prefix: Some(1),
        probe: true,
        visits: Arc::new(Mutex::new(Vec::new())),
    })])?;
    let index = support::published(p4kx_media::export_with(
        &archive, &output, &options, &providers,
    )?)?;
    let row = input::row(&output, &index)?;
    assert_eq!(
        row.pointer("/bytes/0"),
        Some(&serde_json::json!([b'n'])),
        "only the prefix was used"
    );
    assert_eq!(
        row.pointer("/inputs/0/digest/hex"),
        Some(&serde_json::json!(
            p4kx_extraction::Checksum::Sha256(Sha256::digest(b"needed").into()).hex()
        )),
        "receipt hashes the actual complete materialisation rather than the prefix",
    );
    assert_eq!(
        row.pointer("/inputs/0/source/size"),
        Some(&serde_json::json!(6))
    );
    assert_eq!(
        row.pointer("/inputs/0/source/raw"),
        Some(&serde_json::Value::Null)
    );
    Ok(())
}
