// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{ConflictPolicy, Wem};
use p4kx_media::{ExportOptions, ExportOutcome, Index, MediaError, MediaKind, MediaRecord, Status};
use p4kx_testkit::{TestCodec, TestEntry, TestWem};
use serde_json::{Value, json};

use super::{layout_health, support};
use support::TestResult;

const WEM_PATH: &str = "audio.wem";
const AUDIO_STREAM: &str = "audio.jsonl";

#[test]
fn malformed_wem_tail_remains_raw_and_never_replaces_the_complete_generation() -> TestResult {
    let fixture = tempfile::tempdir()?;
    let normal_root = fixture.path().join("normal-source");
    let malformed_root = fixture.path().join("malformed-source");
    fs::create_dir(&normal_root)?;
    fs::create_dir(&malformed_root)?;
    let valid = TestWem::pcm(TestCodec::Pcm, 1, 24_000, &[10, 20]).bytes();
    let malformed = short_trailing_header(&valid)?;
    assert_eq!(
        Wem::parse(malformed.as_slice())?.sample_count(),
        Some(2),
        "the individual WEM summary uses the valid prefix up to first DATA"
    );
    let normal = support::archive_with(
        &normal_root,
        [TestEntry::stored(WEM_PATH, valid.as_slice())],
    )?;
    let broken = support::archive_with(
        &malformed_root,
        [TestEntry::stored(WEM_PATH, malformed.as_slice())],
    )?;
    let output = fixture.path().join("out");
    let index_path = output.join("media-index.json");
    let options = audio_options();
    support::published(p4kx_media::export(&normal, &output, &options)?)?;
    let complete = fs::read(&index_path)?;

    let strict = p4kx_media::export(&broken, &output, &options)
        .err()
        .ok_or("a terminal short chunk header must not publish as complete")?;
    require_projection_loss(&strict);
    assert_eq!(
        fs::read(&index_path)?,
        complete,
        "normal failure preserves the root index"
    );

    let options = ExportOptions {
        diagnostic: true,
        ..options
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&broken, &output, &options)?
    else {
        return Err("malformed RIFF retained only as an explicitly incomplete generation".into());
    };
    let generation = diagnostic.parent().ok_or("diagnostic index parent")?;
    assert!(
        diagnostic.is_file() && !index.complete,
        "durable incomplete index"
    );
    assert_eq!(
        fs::read(&index_path)?,
        complete,
        "diagnostic output preserves the root index"
    );
    require_retained_tail(generation, &index, &malformed)
}

fn require_projection_loss(error: &MediaError) {
    assert!(
        matches!(
            layout_health::leaf(error),
            MediaError::ProjectionLoss {
                kind: MediaKind::Audio,
                ..
            }
        ),
        "incomplete RIFF coverage is a typed projection failure"
    );
}

fn retained_audio_rows(
    output: &Path,
    index: &Index,
) -> Result<Vec<MediaRecord>, Box<dyn std::error::Error>> {
    Ok(support::records(output, index, AUDIO_STREAM)?
        .into_iter()
        .filter(|row| row.source.raw.is_some())
        .collect())
}

fn require_retained_tail(generation: &Path, index: &Index, malformed: &[u8]) -> TestResult {
    let rows = retained_audio_rows(generation, index)?;
    let [header, format, data, failure] = rows.as_slice() else {
        return Err(
            "the valid header and two directory rows precede the exact tail failure".into(),
        );
    };
    assert_eq!(
        header.subresource.namespace, "audio-wem",
        "readable summary survives"
    );
    assert_eq!(format.subresource.indices, [0], "first original chunk");
    assert_eq!(data.subresource.indices, [1], "second original chunk");
    assert_eq!(
        failure.status,
        Status::Failed,
        "short tail has a failed status"
    );
    assert_eq!(
        failure.data,
        json!({"record_type":"wem-failure", "payload":{
            "phase":"chunk-directory",
            "reason":{"type":"truncated", "available":malformed.len().to_string(), "needed":(malformed.len()+4).to_string()}
        }}),
        "tail diagnosis retains precise bytes available and required"
    );
    let raw = failure
        .source
        .raw
        .as_ref()
        .ok_or("retained malformed WEM")?;
    assert_eq!(
        fs::read(generation.join(&index.artifact_base).join(&raw.path))?.as_slice(),
        malformed,
        "exact malformed tail remains recoverable from the raw artifact"
    );
    require_schema(header, format, failure)
}

#[test]
fn opaque_plugin_and_midi_wems_retain_raw_without_a_fake_riff_summary() -> TestResult {
    let fixture = tempfile::tempdir()?;
    let entries: [(&str, &[u8], &str); 3] = [
        ("plugin.wem", b"PLUGopaque", "plugin-media"),
        ("midi.wem", b"MThdopaque", "midi-media"),
        ("rmid.wem", b"RIFF\x04\0\0\0RMID", "midi-media"),
    ];
    let archive = support::archive_with(
        fixture.path(),
        entries.map(|(name, bytes, _)| TestEntry::stored(name, bytes)),
    )?;
    let output = fixture.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &audio_options())?)?;
    let records = retained_audio_rows(&output, &index)?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    assert_eq!(
        records.len(),
        entries.len(),
        "only a class row for each opaque source"
    );
    for (record, (_, bytes, class)) in records.into_iter().zip(entries) {
        assert_eq!(
            record.status,
            Status::Complete,
            "opaque inventory is the advertised scope"
        );
        assert_eq!(
            record.subresource.namespace, "audio-wem-opaque",
            "opaque source namespace"
        );
        assert_eq!(
            record.data,
            json!({"record_type":"wem-opaque","payload":{"class":class}}),
            "original marker determines the opaque type"
        );
        let raw = record.source.raw.as_ref().ok_or("retained opaque WEM")?;
        assert_eq!(
            fs::read(output.join(&index.artifact_base).join(&raw.path))?.as_slice(),
            bytes,
            "opaque Entry round-trips unchanged"
        );
        assert!(
            validator.is_valid(&serde_json::to_value(record)?),
            "opaque record satisfies the same built-in schema"
        );
    }
    Ok(())
}

fn short_trailing_header(valid: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut malformed = valid.to_vec();
    malformed.extend_from_slice(b"JUNK");
    let declared = u32::try_from(
        malformed
            .len()
            .checked_sub(8)
            .ok_or("fixture header length")?,
    )?;
    malformed
        .get_mut(4..8)
        .ok_or("fixture RIFF size")?
        .copy_from_slice(&declared.to_le_bytes());
    Ok(malformed)
}

fn audio_options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Audio],
        conflict: ConflictPolicy::Overwrite,
        ..ExportOptions::default()
    }
}

fn require_schema(header: &MediaRecord, chunk: &MediaRecord, failure: &MediaRecord) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in [header, chunk, failure] {
        assert!(
            validator.is_valid(&serde_json::to_value(row)?),
            "actual Audio row validates"
        );
    }
    for (row, pointer, wrong) in [
        (header, "/data/record_type", json!("wem-chunk")),
        (header, "/data/payload/sample_count", json!(2)),
        (chunk, "/data/payload/tag", json!([1, 2, 3])),
        (chunk, "/subresource/indices", json!([])),
        (failure, "/status", json!("complete")),
        (failure, "/issues", json!([])),
    ] {
        let mut invalid = serde_json::to_value(row)?;
        *invalid
            .pointer_mut(pointer)
            .ok_or("audio mutation address")? = wrong;
        assert!(
            !validator.is_valid(&invalid),
            "schema rejects changed Audio field {pointer}"
        );
    }
    let mut missing = serde_json::to_value(chunk)?;
    let removed = missing
        .pointer_mut("/data/payload")
        .and_then(Value::as_object_mut)
        .and_then(|payload| payload.remove("padding_range"));
    assert_eq!(
        removed,
        Some(Value::Null),
        "the original even chunk has explicit absent padding"
    );
    assert!(
        !validator.is_valid(&missing),
        "nullable padding_range must still be present"
    );
    Ok(())
}
