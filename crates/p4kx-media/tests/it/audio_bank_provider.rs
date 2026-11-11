// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{
    ExportOptions, ExportOutcome, Identity, MediaError, MediaKind, MediaRecord, Status,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::json;

use super::{layout_health, support};

#[path = "audio_bank_provider_fixture.rs"]
mod fixture;
#[path = "audio_bank_provider_origins.rs"]
mod origins;

use self::fixture::{BankFixture, MEDIA_ID, OBJECT_ID};

type BankTestResult<T = ()> = Result<T, Box<dyn Error>>;

const AUDIO_STREAM: &str = "audio.jsonl";
const DUPLICATE_PATH: &str = "bank.BNK";
const DATA_SECTION: usize = 4;

#[test]
fn physical_bnk_sections_duplicate_ids_and_actual_data_ranges_round_trip() -> BankTestResult {
    let folder = tempfile::tempdir()?;
    let fixtures = [
        fixture::fixture(70, 101, b"PLUG sample one", false)?,
        fixture::fixture(71, 102, b"MThd second fixture", false)?,
    ];
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            DUPLICATE_PATH,
            fixtures[0].bytes.as_slice(),
        ))
        .entry(TestEntry::zstd(DUPLICATE_PATH, fixtures[1].bytes.as_slice()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &audio_options())?)?;
    let directory = output.join(&index.artifact_base);
    let records = support::records(&output, &index, AUDIO_STREAM)?;
    let origins = origins::Origins::read(&directory)?;
    require_schema(&records)?;
    for (owner, fixture) in fixtures.iter().enumerate() {
        let source_rows = records.iter().enumerate().filter(|(_, record)| {
            matches!(&record.source.identity, Identity::Entry { entry, .. } if entry.entry_index == owner)
        }).collect::<Vec<_>>();
        check_raw_header(&directory, &source_rows, fixture, owner)?;
        check_sections(&source_rows, fixture)?;
        check_media(&source_rows, fixture, owner, &origins)?;
        check_hirc(&source_rows)?;
    }
    Ok(())
}

#[test]
fn repeated_unique_data_sections_are_retained_and_references_stay_ambiguous() -> BankTestResult {
    let folder = tempfile::tempdir()?;
    let fixture = fixture::fixture(70, 101, b"PLUG sample", true)?;
    let archive = support::archive_with(
        folder.path(),
        [TestEntry::stored(
            "duplicate-data.bnk",
            fixture.bytes.as_slice(),
        )],
    )?;
    let output = folder.path().join("out");
    let options = audio_options();
    let strict = p4kx_media::export(&archive, &output, &options)
        .err()
        .ok_or("duplicate recognized DATA must not publish a complete generation")?;
    assert!(
        matches!(
            layout_health::leaf(&strict),
            MediaError::ProjectionLoss {
                kind: MediaKind::Audio,
                ..
            }
        ),
        "recognized duplicate sections produce typed Audio projection loss"
    );
    let options = ExportOptions {
        diagnostic: true,
        ..options
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("expected diagnostic duplicate DATA inventory".into());
    };
    let directory = diagnostic.parent().ok_or("diagnostic directory")?;
    let records = support::records(directory, &index, AUDIO_STREAM)?;
    require_schema(&records)?;
    check_duplicate_data(&records)?;
    assert!(
        !output.join("media-index.json").exists() && diagnostic.is_file(),
        "only the diagnostic index is published"
    );
    let raw = records
        .iter()
        .find_map(|row| row.source.raw.as_ref())
        .ok_or("retained duplicate bank")?;
    assert_eq!(
        fs::read(directory.join(&index.artifact_base).join(&raw.path))?,
        fixture.bytes,
        "both DATA bodies remain recoverable"
    );
    Ok(())
}

fn check_raw_header(
    directory: &Path,
    source_rows: &[(usize, &MediaRecord)],
    fixture: &BankFixture<'_>,
    owner: usize,
) -> BankTestResult {
    let header = source_rows
        .iter()
        .find(|(_, record)| record.subresource.namespace == "audio-bank")
        .map(|(_, record)| *record)
        .ok_or("actual BKHD row")?;
    let raw = header
        .source
        .raw
        .as_ref()
        .ok_or("usable bank raw artifact")?;
    assert_eq!(
        fs::read(directory.join(&raw.path))?,
        fixture.bytes,
        "raw preserves this physical bank"
    );
    assert_eq!(
        header.data.pointer("/payload/version"),
        Some(&json!(154)),
        "physical version word"
    );
    assert_eq!(
        header.data.pointer("/payload/bank_id"),
        Some(&json!(70 + owner)),
        "physical bank ID"
    );
    assert_eq!(
        header.data.pointer("/payload/language_id"),
        Some(&json!(101 + owner)),
        "physical language ID"
    );
    Ok(())
}

fn check_sections(
    source_rows: &[(usize, &MediaRecord)],
    fixture: &BankFixture<'_>,
) -> BankTestResult {
    let sections = source_rows
        .iter()
        .filter(|(_, record)| record.subresource.namespace == "audio-bank-section")
        .collect::<Vec<_>>();
    assert_eq!(
        sections.len(),
        fixture.sections.len(),
        "all physical sections survive, including opaque repeats"
    );
    for (ordinal, ((_, row), expected)) in sections.into_iter().zip(&fixture.sections).enumerate() {
        assert_eq!(
            row.subresource.indices,
            [u64::try_from(ordinal)?],
            "original section ordinal"
        );
        assert_eq!(
            row.status,
            Status::Complete,
            "repeat of an opaque section is valid"
        );
        assert_eq!(
            row.data.pointer("/payload/tag"),
            Some(&json!(expected.tag)),
            "all four original tag bytes"
        );
        assert_eq!(
            row.data.pointer("/payload/header_range/start"),
            Some(&json!(expected.header.start.to_string())),
            "exact section header offset"
        );
        assert_eq!(
            row.data.pointer("/payload/body_range/end_exclusive"),
            Some(&json!(expected.body.end.to_string())),
            "exact section body end"
        );
        assert_eq!(
            row.data.pointer("/payload/duplicate_unique"),
            Some(&json!(false)),
            "opaque repetitions are not unique-section failures"
        );
    }
    Ok(())
}

fn check_media(
    source_rows: &[(usize, &MediaRecord)],
    fixture: &BankFixture<'_>,
    owner: usize,
    origins: &origins::Origins,
) -> BankTestResult {
    let data = fixture
        .sections
        .get(DATA_SECTION)
        .ok_or("original DATA section")?;
    let mut expected_start = data.body.start;
    let rows = source_rows
        .iter()
        .filter(|(_, record)| record.subresource.namespace == "audio-bank-media")
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        fixture.media.len(),
        "both repeated media ID declarations survive"
    );
    for (position, ((ordinal, row), bytes)) in rows.into_iter().zip(fixture.media).enumerate() {
        let end = expected_start + bytes.len();
        let prefetch = position == 1;
        let class = if prefetch {
            "wem"
        } else if owner == 0 {
            "impulse"
        } else {
            "midi"
        };
        assert_eq!(
            row.subresource.indices,
            [1, u64::try_from(position)?],
            "physical DIDX triplet identity"
        );
        assert_eq!(
            row.data.pointer("/payload/media_id"),
            Some(&json!(MEDIA_ID)),
            "duplicate media short IDs are preserved"
        );
        assert_eq!(
            row.data.pointer("/payload/data"),
            Some(&json!({
                "status":"resolved", "section_ordinal":DATA_SECTION.to_string(),
                "range":{"basis":{"type":"entry"}, "start":expected_start.to_string(), "end_exclusive":end.to_string()},
                "class":class, "declared_wem_length":prefetch.then_some("88"), "prefetch":prefetch
            })),
            "resolved range is under this bank's actual DATA section"
        );
        assert_eq!(
            fixture.bytes.get(expected_start..end),
            Some(bytes),
            "independent expected embedded bytes"
        );
        origins.media(*ordinal, owner, expected_start, prefetch)?;
        expected_start = end;
    }
    Ok(())
}

fn check_hirc(source_rows: &[(usize, &MediaRecord)]) -> BankTestResult {
    let objects = source_rows
        .iter()
        .filter(|(_, record)| record.subresource.namespace == "audio-bank-object")
        .collect::<Vec<_>>();
    let [(_, event), (_, unknown)] = objects.as_slice() else {
        return Err("both original HIRC occurrences must survive".into());
    };
    for (position, row, kind, code) in [(0, event, "event", 4), (1, unknown, "other", 0xe2)] {
        assert_eq!(
            row.subresource.indices,
            [2, position],
            "physical HIRC occurrence"
        );
        assert_eq!(
            row.data.pointer("/payload/object_id"),
            Some(&json!(OBJECT_ID)),
            "duplicate object ID retained"
        );
        assert_eq!(
            row.data.pointer("/payload/kind"),
            Some(&json!(kind)),
            "version-selected kind"
        );
        assert_eq!(
            row.data.pointer("/payload/type_code"),
            Some(&json!(code)),
            "exact known and unknown type code"
        );
    }
    let hierarchy = source_rows
        .iter()
        .find(|(_, row)| row.subresource.namespace == "audio-bank-hirc")
        .map(|(_, row)| *row)
        .ok_or("complete HIRC directory")?;
    assert_eq!(
        hierarchy.data.pointer("/payload/declared_objects"),
        Some(&json!(2)),
        "authored HIRC count"
    );
    assert_eq!(
        hierarchy
            .issues
            .first()
            .ok_or("trailing HIRC diagnostic")?
            .code,
        "audio.bank.trailing-hirc-bytes",
        "opaque tail remains separately identified"
    );
    Ok(())
}

fn check_duplicate_data(rows: &[MediaRecord]) -> BankTestResult {
    let sections = rows
        .iter()
        .filter(|row| {
            row.subresource.namespace == "audio-bank-section"
                && row.data.pointer("/payload/tag") == Some(&json!([68, 65, 84, 65]))
        })
        .collect::<Vec<_>>();
    let [first, second] = sections.as_slice() else {
        return Err("both recognized DATA occurrences must be retained".into());
    };
    assert_eq!(
        first.status,
        Status::Complete,
        "original first occurrence survives"
    );
    assert_eq!(
        second.status,
        Status::Partial,
        "recognized repeated DATA is explicit loss"
    );
    assert_eq!(
        second.data.pointer("/payload/duplicate_unique"),
        Some(&json!(true)),
        "second DATA occurrence identifies its conflict"
    );
    let mut media = rows
        .iter()
        .filter(|row| row.subresource.namespace == "audio-bank-media");
    for _ in 0..2 {
        let row = media
            .next()
            .ok_or("original triplet survives the ambiguous DATA section")?;
        assert_eq!(
            row.data.pointer("/payload/data"),
            Some(&json!({"status":"ambiguous","candidates":"2"})),
            "no arbitrary DATA winner"
        );
    }
    assert!(media.next().is_none(), "exact DIDX row count");
    Ok(())
}

fn audio_options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    }
}

fn require_schema(rows: &[MediaRecord]) -> BankTestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        assert!(
            validator.is_valid(&serde_json::to_value(row)?),
            "actual BNK row {} validates",
            row.subresource.namespace
        );
    }
    Ok(())
}
