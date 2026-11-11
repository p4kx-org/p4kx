// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, ByteOrder, OpenOptions, Wem};
use p4kx_media::{
    ExportOptions, ExportOutcome, Identity, MediaError, MediaKind, MediaRecord, Status,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::json;

use super::{layout_health, support};

#[path = "audio_bank_embedded_wem.rs"]
mod embedded;
#[path = "audio_wem_annotations_fixture.rs"]
mod fixture;
#[path = "audio_wem_annotations_origins.rs"]
mod origins;
#[path = "audio_wem_pcm_conversion.rs"]
mod pcm_conversion;
#[path = "audio_wem_annotations_values.rs"]
mod values;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const AUDIO_STREAM: &str = "audio.jsonl";
const WEM_PATH: &str = "annotations.WEM";

#[test]
fn cue_and_list_export_original_annotations_in_riff_and_rifx() -> TestResult {
    let folder = tempfile::tempdir()?;
    let fixtures = [
        fixture::fixture(ByteOrder::Little)?,
        fixture::fixture(ByteOrder::Big)?,
    ];
    for original in &fixtures {
        assert_eq!(
            Wem::parse(original.bytes.as_slice())?.sample_count(),
            Some(2),
            "original PCM prefix is valid"
        );
    }
    let archive = mixed_archive(folder.path(), &fixtures)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let base = output.join(&index.artifact_base);
    let records = support::records(&output, &index, AUDIO_STREAM)?;
    let origins = origins::Origins::read(&base)?;
    check_schema(&records)?;
    for (owner, fixture) in fixtures.iter().enumerate() {
        let owned = records.iter().enumerate().filter(|(_, record)| {
            matches!(&record.source.identity, Identity::Entry { entry, .. } if entry.entry_index == owner)
        }).collect::<Vec<_>>();
        let raw = owned
            .first()
            .and_then(|(_, row)| row.source.raw.as_ref())
            .ok_or("actual retained WEM")?;
        assert_eq!(
            fs::read(base.join(&raw.path))?,
            fixture.bytes,
            "entire original WEM survives including undecoded suffixes"
        );
        values::cue(&owned, fixture, &origins, owner)?;
        values::label(&owned, fixture, &origins, owner)?;
        values::note(&owned, fixture, &origins, owner)?;
        values::list(&owned, fixture, &origins, owner)?;
    }
    Ok(())
}

#[test]
fn malformed_cue_and_short_list_item_retain_valid_siblings_in_diagnostics() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = fixture::damaged()?;
    let archive = support::archive_with(
        folder.path(),
        [TestEntry::stored(WEM_PATH, bytes.as_slice())],
    )?;
    let output = folder.path().join("out");
    let options = options();
    let error = p4kx_media::export(&archive, &output, &options)
        .err()
        .ok_or("malformed annotations cannot publish normally")?;
    assert!(
        matches!(
            layout_health::leaf(&error),
            MediaError::ProjectionLoss {
                kind: MediaKind::Audio,
                ..
            }
        ),
        "typed annotation loss"
    );
    let options = ExportOptions {
        diagnostic: true,
        ..options
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("malformed annotation diagnostic generation".into());
    };
    let directory = diagnostic
        .parent()
        .ok_or("diagnostic generation directory")?;
    let rows = support::records(directory, &index, AUDIO_STREAM)?;
    check_damaged(&rows)?;
    check_schema(&rows)?;
    let raw = rows
        .first()
        .and_then(|record| record.source.raw.as_ref())
        .ok_or("exact retained malformed WEM")?;
    assert_eq!(
        fs::read(directory.join(&raw.path))?,
        bytes,
        "both malformed declarations remain byte-exact"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "incomplete report does not become root index"
    );
    Ok(())
}

fn check_damaged(rows: &[MediaRecord]) -> TestResult {
    let cue = row(rows, "audio-wem-cue", &[2])?;
    assert_eq!(
        cue.data.pointer("/payload/declared_points"),
        Some(&json!(2)),
        "authored count survives"
    );
    row(rows, "audio-wem-cue-point", &[2, 0])?;
    assert!(
        rows.iter()
            .all(|row| row.subresource.namespace != "audio-wem-cue-tail"),
        "failed traversal publishes no cue tail"
    );
    let failure = row(rows, "audio-wem-annotation-failure", &[2])?;
    assert_eq!(
        failure.status,
        Status::Failed,
        "cue failure belongs to its actual chunk"
    );
    assert_eq!(
        failure.data.pointer("/payload/phase"),
        Some(&json!("cue")),
        "typed phase"
    );
    let short = row(rows, "audio-wem-list-item", &[3, 0])?;
    assert_eq!(
        short.status,
        Status::Partial,
        "bounded short child retains physical range"
    );
    assert_eq!(
        short.data.pointer("/payload/content"),
        Some(&json!({"type":"malformed","payload":{"available":"2","needed":"4"}})),
        "original short label shape with no fabricated value"
    );
    let later = row(rows, "audio-wem-list-item", &[3, 1])?;
    assert_eq!(
        later.data.pointer("/payload/content/payload/utf8_text"),
        Some(&json!("later")),
        "later sibling remains readable after the structural failure"
    );
    let mut invalid = serde_json::to_value(short)?;
    *invalid.get_mut("status").ok_or("status field")? = json!("complete");
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    assert!(
        !validator.is_valid(&invalid),
        "schema rejects falsely complete malformed child"
    );
    Ok(())
}

fn mixed_archive(directory: &Path, fixtures: &[fixture::Fixture; 2]) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(WEM_PATH, fixtures[0].bytes.as_slice()))
        .entry(TestEntry::zstd(WEM_PATH, fixtures[1].bytes.as_slice()).encrypted())
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

fn row<'record>(
    rows: &'record [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> TestResult<&'record MediaRecord> {
    rows.iter()
        .find(|row| {
            row.subresource.namespace == namespace && row.subresource.indices.as_slice() == indices
        })
        .ok_or_else(|| format!("missing {namespace} at {indices:?}").into())
}

fn check_schema(rows: &[MediaRecord]) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        assert!(
            validator.is_valid(&serde_json::to_value(row)?),
            "{} record validates",
            row.subresource.namespace
        );
    }
    Ok(())
}
