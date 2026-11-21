// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestContainer, TestEntry, test_key};
use serde_json::{Value, json};

use crate::support;
use crate::support::audio_provider::{
    CONTAINER, Origins, Owner, TestResult, check_identity, wem_bytes,
};

const LOCAL_WEM: &str = "Data/Sounds/WwIsE/MEDIA/German/42.WEM";
const NEUTRAL_WEM: &str = "Data/Sounds/wwise/Media/43.wem";
const OTHER_WEM: &str = "Data/Sounds/wwise/Unexpected/German/44.wem";
const LOCAL_BANK: &str = "Data/Sounds/wwise/Czech/Fixture.bnk";
const NEUTRAL_BANK: &str = "Data/Sounds/wwise/Fixture.bnk";
const NESTED_WEM: &str = "Data\\Sounds\\wwise\\Dutch\\Media\\45.wem";
const NAMESPACE: &str = "audio-source-language";

#[test]
fn source_language_retains_duplicate_and_neutral_placements_with_directory_evidence() -> TestResult
{
    let root = tempfile::tempdir()?;
    let archive = fixture(root.path())?;
    let destination = root.path().join("media");
    let index = support::published(p4kx_media::export(
        &archive,
        &destination,
        &ExportOptions {
            kinds: vec![MediaKind::Audio],
            ..ExportOptions::default()
        },
    )?)?;
    let records = support::records(&destination, &index, "audio.jsonl")?;
    let origins = Origins::read(&destination.join(&index.artifact_base))?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let placements: Vec<_> = records
        .iter()
        .enumerate()
        .filter(|(_, record)| record.subresource.namespace == NAMESPACE)
        .collect();
    let expected = [
        (owner(LOCAL_WEM, 0, false), Some("German")),
        (owner(LOCAL_WEM, 1, true), Some("German")),
        (owner(NEUTRAL_WEM, 2, false), None),
        (owner(LOCAL_BANK, 4, true), Some("Czech")),
        (owner(NEUTRAL_BANK, 5, false), None),
        (nested_owner(), Some("Dutch")),
    ];
    assert_eq!(
        placements.len(),
        expected.len(),
        "one row per known placement"
    );
    for (owner, language) in expected {
        let (ordinal, row) = placements
            .iter()
            .copied()
            .find(|(_, row)| owns(row, owner))
            .ok_or_else(|| format!("known physical placement {} at {}", owner.path, owner.entry))?;
        check_row(row, owner, language, &validator)?;
        check_folder_evidence(&origins, ordinal, owner)?;
    }
    assert!(
        records
            .iter()
            .any(|row| row.subresource.namespace == "audio-wem"
                && owns(row, owner(OTHER_WEM, 3, false))),
        "an unclassified WEM is retained without a placement row"
    );
    check_numeric_bank_language(&records, &origins)
}

fn fixture(root: &Path) -> TestResult<Archive> {
    let path = root.join("Data.p4k");
    let bytes = wem_bytes();
    let bank = TestBank::new(100).bytes();
    let nested = TestContainer::new().entry(TestEntry::stored(NESTED_WEM, bytes.as_slice()));
    ArchiveBuilder::new()
        .entry(TestEntry::stored(LOCAL_WEM, bytes.as_slice()))
        .entry(TestEntry::zstd(LOCAL_WEM, bytes.as_slice()).encrypted())
        .entry(TestEntry::stored(NEUTRAL_WEM, bytes.as_slice()))
        .entry(TestEntry::stored(OTHER_WEM, bytes.as_slice()))
        .entry(TestEntry::stored(LOCAL_BANK, bank.as_slice()).encrypted())
        .entry(TestEntry::stored(NEUTRAL_BANK, bank.as_slice()))
        .entry(TestEntry::stored(CONTAINER, nested.build()).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

const fn owner(path: &'static str, entry: usize, encrypted: bool) -> Owner {
    Owner {
        path,
        entry,
        parent: None,
        stored: if encrypted {
            "encrypted"
        } else {
            "unencrypted"
        },
        metadata: false,
        payload: encrypted,
    }
}

const fn nested_owner() -> Owner {
    Owner {
        parent: Some(6),
        stored: "unknown",
        metadata: true,
        payload: true,
        ..owner(NESTED_WEM, 0, false)
    }
}

fn owns(record: &MediaRecord, owner: Owner) -> bool {
    let Identity::Entry { entry, .. } = &record.source.identity else {
        return false;
    };
    (entry.path.as_str(), entry.entry_index) == (owner.path, owner.entry)
}

fn check_row(
    record: &MediaRecord,
    owner: Owner,
    language: Option<&str>,
    validator: &jsonschema::Validator,
) -> TestResult {
    check_identity(&record.source, owner)?;
    assert!(
        record.subresource.indices.is_empty(),
        "placement describes the Entry"
    );
    assert_eq!(
        record.data,
        json!({"record_type":"source-language","payload":{"language_folder":language}}),
        "known neutral placement has an explicit null"
    );
    let mut wire = serde_json::to_value(record)?;
    assert!(validator.is_valid(&wire), "actual placement validates");
    let removed = wire
        .pointer_mut("/data/payload")
        .and_then(Value::as_object_mut)
        .and_then(|payload| payload.remove("language_folder"));
    assert!(removed.is_some(), "language_folder was serialized");
    assert!(!validator.is_valid(&wire), "language_folder is mandatory");
    Ok(())
}

fn check_folder_evidence(origins: &Origins, ordinal: usize, owner: Owner) -> TestResult {
    let evidence = origins.field(
        "records",
        ordinal,
        "/data/payload/language_folder",
        owner.metadata,
    )?;
    let uses = origins.require_owner(evidence, owner)?;
    let [source_use] = uses.as_slice() else {
        return Err("one actual source-path contribution".into());
    };
    assert_eq!(
        source_use.get("scope"),
        Some(&json!("archiveDirectoryMetadata")),
        "language folder comes from the directory"
    );
    assert_eq!(
        source_use.get("role"),
        Some(&json!("conversion")),
        "language folder is derived from the path"
    );
    assert_eq!(
        source_use.get("location"),
        Some(
            &json!({"type":"subresource","namespace":"archive-directory","indices":[],"field":"path"})
        ),
        "actual directory field"
    );
    Ok(())
}

fn check_numeric_bank_language(records: &[MediaRecord], origins: &Origins) -> TestResult {
    let owner = owner(LOCAL_BANK, 4, true);
    let (ordinal, bank) = records
        .iter()
        .enumerate()
        .find(|(_, row)| row.subresource.namespace == "audio-bank" && owns(row, owner))
        .ok_or("original numeric BKHD language")?;
    assert_eq!(
        bank.data.pointer("/payload/language_id"),
        Some(&json!(0x1770_5D3E)),
        "BKHD language ID stays independent of the physical folder"
    );
    let evidence = origins.field(
        "records",
        ordinal,
        "/data/payload/language_id",
        owner.payload,
    )?;
    let uses = origins.require_owner(evidence, owner)?;
    assert!(
        uses.iter()
            .any(|source_use| source_use.get("scope") == Some(&json!("payload"))),
        "BKHD language ID comes from the encrypted payload"
    );
    Ok(())
}
