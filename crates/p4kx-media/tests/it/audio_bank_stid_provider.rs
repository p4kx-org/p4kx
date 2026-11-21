// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaError, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::json;

use super::{layout_health, support};

#[path = "audio_bank_stid_fixture.rs"]
mod fixture;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const AUDIO_STREAM: &str = "audio.jsonl";
const NAME_NAMESPACE: &str = "audio-bank-name";

#[test]
fn stid_keeps_duplicate_ids_exact_bytes_and_utf8_states() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = fixture::soundbank(4, &fixture::NAMES, fixture::TRAILING)?;
    let archive_path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("names.bnk", bytes.as_slice()).encrypted())
        .write_to(&archive_path)?;
    let archive = Archive::open_with(archive_path, OpenOptions::new().key(Some(test_key())))?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &audio_options())?)?;
    let base = output.join(&index.artifact_base);
    let records = support::records(&output, &index, AUDIO_STREAM)?;
    let summary = row(&records, "audio-bank-stid")?;
    assert_eq!(
        summary.data,
        json!({"record_type":"bank-stid","payload":{"section_ordinal":"1","string_type":fixture::STRING_TYPE,"declared_names":4}}),
        "raw string type and count survive"
    );
    let raw = summary.source.raw.as_ref().ok_or("verified BNK artifact")?;
    assert_eq!(
        fs::read(base.join(&raw.path))?,
        bytes,
        "exact original STID bytes retained"
    );
    let names = records
        .iter()
        .enumerate()
        .filter(|(_, record)| record.subresource.namespace == NAME_NAMESPACE)
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        fixture::NAMES.len(),
        "all physical names survive"
    );
    let second = names.get(1).ok_or("second duplicate ID")?;
    fixture::check_origins(&base, second.0)?;
    check_names(&names, &bytes)?;
    let tail = row(&records, "audio-bank-stid-tail")?;
    assert_eq!(
        tail.data.pointer("/payload/trailing_range"),
        Some(&fixture::range(
            bytes.len() - fixture::TRAILING.len(),
            bytes.len()
        )),
        "post-name raw suffix remains addressable"
    );
    check_schema(&records)
}

#[test]
fn truncated_stid_keeps_declared_count_and_readable_prefix_in_diagnostics() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = fixture::soundbank(2, &[(5, b"ok")], &[6, 0, 0, 0, 5, b'x', b'y'])?;
    let archive = support::archive_with(
        folder.path(),
        [TestEntry::stored("truncated.bnk", bytes.as_slice())],
    )?;
    let output = folder.path().join("out");
    let options = audio_options();
    let failure = p4kx_media::export(&archive, &output, &options)
        .err()
        .ok_or("truncated STID is not complete")?;
    assert!(
        matches!(
            layout_health::leaf(&failure),
            MediaError::ProjectionLoss {
                kind: MediaKind::Audio,
                ..
            }
        ),
        "typed Audio projection loss"
    );
    let options = ExportOptions {
        diagnostic: true,
        ..options
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("STID failure belongs in an incomplete generation".into());
    };
    let directory = diagnostic.parent().ok_or("diagnostic base directory")?;
    let records = support::records(directory, &index, AUDIO_STREAM)?;
    check_truncation(&records)?;
    let failure = row(&records, "audio-bank-failure")?;
    let raw = failure
        .source
        .raw
        .as_ref()
        .ok_or("exact retained malformed soundbank")?;
    assert_eq!(
        fs::read(directory.join(&raw.path))?,
        bytes,
        "invalid suffix survives unchanged"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "strict publication never advertises incomplete STID"
    );
    check_schema(&records)
}

fn check_truncation(rows: &[MediaRecord]) -> TestResult {
    let authored = row(rows, "audio-bank-stid")?;
    assert_eq!(
        authored.data.pointer("/payload/declared_names"),
        Some(&json!(2)),
        "preserve original count, not observed count"
    );
    let prefix = row(rows, NAME_NAMESPACE)?;
    assert_eq!(
        prefix.data.pointer("/payload/name_text/text"),
        Some(&json!("ok")),
        "complete prefix name survives"
    );
    let failure = row(rows, "audio-bank-failure")?;
    assert_eq!(
        failure.status,
        Status::Failed,
        "failed names phase is explicit"
    );
    assert_eq!(
        failure.data,
        json!({"record_type":"bank-failure","payload":{"phase":"names","section_ordinal":"1",
            "reason":{"type":"truncated","section":"STID","available":"22","needed":"25"}}}),
        "diagnosis gives exact original section-local bounds"
    );
    assert!(
        rows.iter()
            .all(|record| record.subresource.namespace != "audio-bank-stid-tail"),
        "an incomplete scan has no successful trailing range"
    );
    Ok(())
}

fn check_names(names: &[(usize, &MediaRecord)], bank: &[u8]) -> TestResult {
    let mut start = fixture::FIRST_NAME_HEADER;
    let values = [
        json!({"encoding":"utf8","text":" a\0name "}),
        json!({"encoding":"invalid-utf8","valid_up_to":2,"error_length":1}),
        json!({"encoding":"invalid-utf8","valid_up_to":0,"error_length":null}),
        json!({"encoding":"utf8","text":""}),
    ];
    for (position, (((_, row), (id, bytes)), expected_text)) in
        names.iter().zip(fixture::NAMES).zip(values).enumerate()
    {
        let name = start + fixture::NAME_HEADER_BYTES;
        let end = name + bytes.len();
        assert_eq!(bank.get(name..end), Some(bytes), "fixture's original bytes");
        assert_eq!(
            row.subresource.indices,
            [1, u64::try_from(position)?],
            "physical ordinal preserves duplicates"
        );
        assert_eq!(
            row.status,
            Status::Complete,
            "original bytes and interpretation state are available"
        );
        assert_eq!(
            row.data,
            json!({"record_type":"bank-name","payload":{
                "section_ordinal":"1","ordinal":position.to_string(),"bank_id":id,
                "name_length":bytes.len(),"header_range":fixture::range(start,name),
                "name_range":fixture::range(name,end),"name_bytes":bytes,"name_text":expected_text}}),
            "complete original name fields and precise UTF-8 result"
        );
        start = end;
    }
    Ok(())
}

fn audio_options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    }
}

fn row<'a>(records: &'a [MediaRecord], namespace: &str) -> TestResult<&'a MediaRecord> {
    let mut matching = records
        .iter()
        .filter(|record| record.subresource.namespace == namespace);
    let record = matching.next().ok_or("expected one exact record")?;
    assert!(
        matching.next().is_none(),
        "exact occurrence count for {namespace}"
    );
    Ok(record)
}

fn check_schema(records: &[MediaRecord]) -> TestResult {
    let schema = p4kx_media::builtin_record_schema()?;
    let validator = jsonschema::validator_for(&schema)?;
    for record in records {
        assert!(
            validator.is_valid(&serde_json::to_value(record)?),
            "{} record validates",
            record.subresource.namespace
        );
    }
    let name = records
        .iter()
        .find(|record| record.subresource.namespace == NAME_NAMESPACE)
        .ok_or("at least one STID name")?;
    let mut invalid = serde_json::to_value(name)?;
    *invalid
        .pointer_mut("/data/payload/name_bytes")
        .ok_or("original byte array")? = json!(vec![0_u8; 256]);
    assert!(
        !validator.is_valid(&invalid),
        "schema rejects names outside the original byte width"
    );
    Ok(())
}
