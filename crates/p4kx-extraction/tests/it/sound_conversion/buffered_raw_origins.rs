// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{
    AudioConversion, Checksum, ConflictPolicy, Exclusion, ExtractOptions, ExtractionReport,
    FolderSource, convert_folder, extract,
};
use p4kx_testkit::{TestBank, TestEntry};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::bank_origins::expect_origin;
use super::fixture::{BANK, Fixture, TestResult, XMA, partial_files, pcm, xma};
use crate::bulk_wav_origin::{field_uses, read_companion};

const BANK_PATH: &str = "Data/Sounds/wwise/Ship.bnk";
const XMA_PATH: &str = "Data/Sounds/wwise/Media/700.wem";
const MIDI_ENTRY: &str = "Data/Sounds/wwise/Media/800.wem";
const MIDI_PATH: &str = "Data/Sounds/wwise/Media/800.mid";
const MIDI: &[u8] = b"MThd\0\0\0\x06\0\x01\0\x01\x01\xE0";

#[test]
fn archive_conversion_proves_the_parent_bank_and_unsupported_loose_audio() -> TestResult {
    let bank = TestBank::new(1).wem(4, pcm()).bytes();
    let unsupported = xma();
    let fixture = Fixture::new(vec![
        TestEntry::zstd(BANK, bank.as_slice()).encrypted(),
        TestEntry::zstd(XMA, unsupported.as_slice()),
    ])?;
    let report = fixture.run(AudioConversion::Ogg, ConflictPolicy::Overwrite);

    assert!(report.failures().is_empty(), "{report:?}");
    assert_eq!(report.extracted(), 2, "both inputs produced output");
    assert_eq!(
        report.converted(),
        Some(1),
        "only the bank member converted"
    );
    assert_eq!(report.unconverted().len(), 1, "unsupported WEM is reported");
    assert_eq!(report.unconverted()[0].path(), XMA);
    assert_eq!(report.raw_artifacts().len(), 2);
    check_original(&fixture.target, &report, BANK_PATH, &bank, &json!(true))?;
    check_original(
        &fixture.target,
        &report,
        XMA_PATH,
        &unsupported,
        &json!(false),
    )
}

#[test]
fn folder_conversion_proves_renamed_midi_and_a_malformed_bank() -> TestResult {
    let folder = tempfile::tempdir()?;
    let input = folder.path().join("in");
    let target = folder.path().join("out");
    let bank = b"BKHD broken";
    for (name, bytes) in [(MIDI_ENTRY, MIDI), (BANK_PATH, bank.as_slice())] {
        let path = input.join(name);
        fs::create_dir_all(path.parent().ok_or("input folder")?)?;
        fs::write(path, bytes)?;
    }
    let source = FolderSource::scan(&input, None)?;
    let selected = source.select(None, &Exclusion::default());
    let report = convert_folder(
        &source,
        &selected,
        false,
        &target,
        &ExtractOptions {
            convert: true,
            audio: AudioConversion::Ogg,
            ..ExtractOptions::default()
        },
    )?;

    assert!(report.failures().is_empty(), "{report:?}");
    assert_eq!(report.extracted(), 2);
    assert_eq!(report.converted(), Some(0));
    assert_eq!(
        report.unconverted().len(),
        1,
        "only the invalid bank fails parsing"
    );
    assert!(
        !target.join(MIDI_ENTRY).exists(),
        "MIDI uses the .mid output"
    );
    assert_eq!(report.raw_artifacts().len(), 2);
    check_original(&target, &report, MIDI_PATH, MIDI, &Value::Null)?;
    check_original(&target, &report, BANK_PATH, bank, &Value::Null)
}

#[test]
fn an_unchanged_parent_bank_and_fallback_wem_get_no_retroactive_receipts() -> TestResult {
    let bank = TestBank::new(1).bytes();
    let unsupported = xma();
    let fixture = Fixture::new(vec![
        TestEntry::stored(BANK, bank.as_slice()),
        TestEntry::stored(XMA, unsupported.as_slice()),
    ])?;
    for (name, bytes) in [
        (BANK_PATH, bank.as_slice()),
        (XMA_PATH, unsupported.as_slice()),
    ] {
        let path = fixture.target.join(name);
        fs::create_dir_all(path.parent().ok_or("output folder")?)?;
        fs::write(path, bytes)?;
    }
    for policy in [ConflictPolicy::Skip, ConflictPolicy::IfChanged] {
        let report = fixture.run(AudioConversion::Ogg, policy);
        assert_eq!(report.extracted(), 0, "{report:?}");
        assert_eq!(report.skipped().len(), 2, "both incumbents are kept");
        assert!(report.raw_artifacts().is_empty(), "no new publication");
        for name in [BANK_PATH, XMA_PATH] {
            assert!(!fixture.exists(&format!("{name}.p4kx-origin.jsonl")));
        }
    }
    Ok(())
}

#[test]
fn unreadable_archive_audio_leaves_neither_an_artifact_nor_a_companion() -> TestResult {
    let fixture = Fixture::new(vec![
        TestEntry::deflated(BANK, TestBank::new(1).bytes()).with_payload(vec![0xff; 8]),
        TestEntry::deflated(XMA, xma()).with_payload(vec![0xff; 8]),
    ])?;
    let report = fixture.run(AudioConversion::Ogg, ConflictPolicy::Overwrite);

    assert_eq!(report.failures().len(), 2, "{report:?}");
    assert_eq!(report.extracted(), 0);
    assert!(
        report.unconverted().is_empty(),
        "failure to read precedes conversion"
    );
    assert!(report.raw_artifacts().is_empty(), "no admitted bytes");
    for name in [BANK_PATH, XMA_PATH] {
        assert!(!fixture.exists(name));
        assert!(!fixture.exists(&format!("{name}.p4kx-origin.jsonl")));
    }
    assert!(
        partial_files(&fixture.target)?.is_empty(),
        "no staged files"
    );
    Ok(())
}

#[test]
fn a_later_explicit_midi_replaces_the_generated_output_and_its_receipt() -> TestResult {
    let native = b"explicit MIDI Entry";
    let fixture = Fixture::new(vec![
        TestEntry::stored(MIDI_ENTRY, MIDI),
        TestEntry::stored(MIDI_PATH, native.as_slice()),
    ])?;
    let entries = [MIDI_ENTRY, MIDI_PATH].map(|path| {
        fixture
            .archive
            .entries()
            .iter()
            .find(|entry| entry.path() == path)
    });
    let [Some(input), Some(explicit)] = entries else {
        return Err("the original WEM and explicit MIDI Entries".into());
    };
    let report = extract(
        &fixture.archive,
        &[input, explicit],
        &fixture.target,
        &ExtractOptions {
            convert: true,
            audio: AudioConversion::Ogg,
            policy: ConflictPolicy::Overwrite,
            ..ExtractOptions::default()
        },
    );
    assert!(report.is_complete(), "{report:?}");
    assert_eq!(
        report.raw_artifacts().len(),
        1,
        "only the final output is reported"
    );
    check_original(&fixture.target, &report, MIDI_PATH, native, &json!(false))
}

fn check_original(
    target: &Path,
    report: &ExtractionReport,
    path: &str,
    bytes: &[u8],
    encryption: &Value,
) -> TestResult {
    let artifact = report
        .raw_artifacts()
        .iter()
        .find(|item| item.path == path)
        .ok_or("reported unchanged output")?;
    assert_eq!(
        fs::read(target.join(path))?,
        bytes,
        "unchanged output bytes"
    );
    let digest = Checksum::Sha256(Sha256::digest(bytes).into()).hex();
    assert_eq!(
        artifact.digest.sha256(),
        digest,
        "reported digest of the unchanged bytes"
    );
    let companion = artifact
        .origin_receipt
        .as_deref()
        .ok_or("output companion")?;
    assert_eq!(
        companion,
        format!("{path}.p4kx-origin.jsonl"),
        "receipt beside the unchanged output"
    );
    let rows = read_companion(&fs::read(target.join(companion))?)?;
    assert_eq!(
        rows.first()
            .and_then(|header| header.pointer("/binding/sha256")),
        Some(&json!(digest)),
        "receipt binds the retained bytes",
    );
    let payload = rows
        .iter()
        .find(|row| row.pointer("/address/type") == Some(&json!("opaquePayload")))
        .ok_or("whole output origin")?;
    assert_eq!(
        &payload["containsEncryptedSource"], encryption,
        "encryption of the selected input source"
    );
    assert_eq!(
        payload["sourceSetComplete"], true,
        "the physical payload source is known"
    );
    let namespace = if encryption.is_null() {
        "standaloneFileBytes"
    } else {
        "archiveEntryDecodedBytes"
    };
    let uses = field_uses(&rows, payload)?;
    assert_eq!(uses.len(), 1, "one physical payload use");
    expect_origin(
        &uses,
        namespace,
        "bytes",
        &[0, bytes.len() as u64],
        "payload",
    );
    Ok(())
}
