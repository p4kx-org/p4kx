// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{
    ExportOptions, ExportOutcome, Identity, MediaError, MediaKind, MediaRecord, Status,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::audio_bank_hirc_action_origins::{self as origins, Origins};
use super::{layout_health, support};

#[path = "audio_bank_hirc_sound_fixture.rs"]
mod fixture;

use self::fixture::{Fixture, Layout};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SAME_NAME: &str = "sound.BNK";
const NAMESPACE: &str = "audio-bank-hirc-sound";
const STREAM: &str = "audio.jsonl";

#[test]
fn sound_export_retains_source_plugin_and_node_words_with_physical_provenance() -> TestResult {
    let folder = tempfile::tempdir()?;
    let banks = [fixture::bank(Layout::Opaque), fixture::bank(Layout::Full)];
    let archive = archive(folder.path(), &banks)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let base = output.join(&index.artifact_base);
    let rows = support::records(&output, &index, STREAM)?;
    let originals = Origins::read(&base)?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for (owner, bank) in banks.iter().enumerate() {
        let (position, row) = physical(&rows, owner)?;
        assert_eq!(row.status, Status::Complete, "fully read Sound");
        assert_eq!(
            row.data.get("record_type"),
            Some(&json!("bank-hirc-sound")),
            "closed Sound row"
        );
        assert_eq!(row.subresource.indices, [1, 0], "physical HIRC occurrence");
        assert!(
            validator.is_valid(&serde_json::to_value(row)?),
            "record schema"
        );
        assert_raw(&base, row, bank)?;
        check_words(&originals, position, owner, row, bank)?;
        if owner == 0 {
            check_older(row, bank);
        } else {
            check_node(row, bank);
            check_parent_inputs(&originals, position, bank)?;
        }
    }
    Ok(())
}

#[test]
fn malformed_sound_keeps_the_readable_length_and_never_invents_a_node() -> TestResult {
    let folder = tempfile::tempdir()?;
    let banks = [
        fixture::bank(Layout::Truncated),
        fixture::bank(Layout::Unsupported),
    ];
    let archive = archive(folder.path(), &banks)?;
    let output = folder.path().join("out");
    let normal = p4kx_media::export(&archive, &output, &options())
        .err()
        .ok_or("unreadable original Sound layout is projection loss")?;
    assert!(
        matches!(
            layout_health::leaf(&normal),
            MediaError::ProjectionLoss {
                kind: MediaKind::Audio,
                ..
            }
        ),
        "typed normal failure"
    );
    let diagnostics = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &diagnostics)?
    else {
        return Err("diagnostic Sound inventory".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic export root")?;
    let records = support::records(base, &index, STREAM)?;
    check_diagnostics(base, &records, &banks)
}

fn check_diagnostics(base: &Path, records: &[MediaRecord], banks: &[Fixture; 2]) -> TestResult {
    let (position, malformed) = physical(records, 0)?;
    assert_eq!(
        malformed.status,
        Status::Partial,
        "original layout is incomplete"
    );
    assert_eq!(
        payload(malformed, "/layout"),
        Some(json!("malformed")),
        "known layout"
    );
    assert_eq!(
        payload(malformed, "/continuation/node_base"),
        Some(Value::Null),
        "node was never reached"
    );
    assert_eq!(
        payload(malformed, "/continuation/plugin_parameters_range"),
        Some(Value::Null),
        "unadmitted extent"
    );
    let origins = Origins::read(base)?;
    check_words(&origins, position, 0, malformed, &banks[0])?;
    assert_raw(base, malformed, &banks[0])?;
    let (_, unsupported) = physical(records, 1)?;
    assert_eq!(
        unsupported.status,
        Status::Complete,
        "complete physical inventory"
    );
    assert_eq!(
        payload(unsupported, "/layout"),
        Some(json!("unsupported")),
        "future layout explicitly diagnosed"
    );
    assert_eq!(
        payload(unsupported, "/source"),
        Some(Value::Null),
        "no guessed field"
    );
    assert_raw(base, unsupported, &banks[1])
}

fn check_words(
    originals: &Origins,
    position: usize,
    owner: usize,
    row: &MediaRecord,
    bank: &Fixture,
) -> TestResult {
    for expected in &bank.expected {
        assert_eq!(
            payload(row, expected.path).as_ref(),
            Some(&expected.value),
            "original {}",
            expected.path
        );
        let path = format!("/data/payload{}", expected.path);
        let uses = originals.uses(position, owner, &path)?;
        assert!(
            origins::has_exact(&uses, expected.name, "payload", &expected.range),
            "{path} points at the original scalar"
        );
    }
    Ok(())
}

fn check_older(row: &MediaRecord, bank: &Fixture) {
    assert_eq!(
        payload(row, "/source/cache_id"),
        Some(Value::Null),
        "no authored cache ID in version 150"
    );
    assert_eq!(
        payload(row, "/source/storage"),
        Some(json!("prefetch-streamed")),
        "known original storage byte"
    );
    assert_eq!(
        payload(row, "/source/external_source"),
        Some(json!(true)),
        "external source plug-in"
    );
    assert_eq!(
        payload(row, "/continuation/opaque_range"),
        Some(range(&bank.continuation_bytes)),
        "original opaque suffix"
    );
}

fn check_node(row: &MediaRecord, bank: &Fixture) {
    assert_eq!(
        payload(row, "/source/storage"),
        Some(json!("other")),
        "unknown storage code is retained"
    );
    assert_eq!(
        payload(row, "/source/language_specific"),
        Some(json!(true)),
        "language bit of original flags"
    );
    assert_eq!(
        payload(row, "/source/source_plugin"),
        Some(json!(true)),
        "generating plug-in"
    );
    assert_eq!(
        payload(row, "/source/has_media_file"),
        Some(json!(false)),
        "source does not reference a WEM"
    );
    assert_eq!(
        payload(row, "/continuation/plugin_parameters_range"),
        Some(range(&bank.continuation_bytes)),
        "exact original plug-in bytes"
    );
    assert_eq!(
        payload(row, "/continuation/node_base/trailing_range"),
        Some(range(&(bank.bytes.len()..bank.bytes.len()))),
        "exact empty node suffix"
    );
}

fn check_parent_inputs(originals: &Origins, position: usize, bank: &Fixture) -> TestResult {
    let uses = originals.uses(
        position,
        1,
        "/data/payload/continuation/node_base/parent_id",
    )?;
    for name in [
        "soundPluginParameterLength",
        "soundNodeEffectCount",
        "soundNodeMetadataCount",
    ] {
        let word = bank
            .expected
            .iter()
            .find(|word| word.name == name)
            .ok_or("selector fixture")?;
        assert!(
            origins::has_exact(&uses, name, "selector", &word.range),
            "parent depends on original {name}"
        );
    }
    assert!(
        !origins::has_field(&uses, "soundNodePreParentWord"),
        "opaque preceding value is not a selector"
    );
    Ok(())
}

fn physical(rows: &[MediaRecord], owner: usize) -> TestResult<(usize, &MediaRecord)> {
    rows.iter().enumerate().find(|(_, row)| {
        row.subresource.namespace == NAMESPACE
            && matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.entry_index == owner)
    }).ok_or_else(|| format!("missing Sound for physical Entry {owner}").into())
}

fn assert_raw(base: &Path, row: &MediaRecord, bank: &Fixture) -> TestResult {
    let raw = row
        .source
        .raw
        .as_ref()
        .ok_or("retained original Sound bank")?;
    assert_eq!(
        fs::read(base.join(&raw.path))?,
        bank.bytes,
        "exact retained original bank"
    );
    Ok(())
}

fn payload(row: &MediaRecord, path: &str) -> Option<Value> {
    row.data.pointer(&format!("/payload{path}")).cloned()
}

fn range(extent: &Range<usize>) -> Value {
    json!({"basis":{"type":"entry"},"start":extent.start.to_string(),"end_exclusive":extent.end.to_string()})
}

fn archive(directory: &Path, banks: &[Fixture; 2]) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SAME_NAME, banks[0].bytes.as_slice()))
        .entry(TestEntry::zstd(SAME_NAME, banks[1].bytes.as_slice()).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    }
}
