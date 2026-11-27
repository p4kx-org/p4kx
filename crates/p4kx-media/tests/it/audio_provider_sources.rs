// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Checksum, Exclusion, Filter, OpenOptions, SourceEncryption, Wem};
use p4kx_media::{Artifact, ByteScope, ExportOptions, Index, MediaKind, Source, Status};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, test_key};
use serde_json::json;
use sha2::{Digest as _, Sha256};

use crate::support;
use crate::support::audio_provider::{
    CONTAINER, Origins, Owner, TestResult, check_identity, read_rows, wem_bytes,
};

#[path = "audio_provider_chunks.rs"]
mod chunks;

const SAME: &str = "Audio/same.WEM";
const CHILD: &str = "Audio\\child.wem";
const WEM_HEADER: &str = "audio-wem";
const FORMAT_BODY: &str = "44";
const DATA_BODY: &str = "68";

#[test]
fn default_audio_provider_projects_selected_loose_wems_with_original_raw_receipts() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = wem_bytes();
    check_original_bytes(&bytes)?;
    let archive = archive(folder.path(), &bytes)?;
    let options = ExportOptions {
        kinds: vec![MediaKind::Audio],
        include: Some(Filter::parse("*.wem")?),
        exclude: Exclusion::parse(["excluded.wem"])?,
        ..ExportOptions::default()
    };

    let mut streams = Vec::new();
    for name in ["first", "second"] {
        let destination = folder.path().join(name);
        let index = support::published(p4kx_media::export(&archive, &destination, &options)?)?;
        check_selection(&index)?;
        let base = destination.join(&index.artifact_base);
        let origins = Origins::read(&base)?;
        check_raw(&base, &index, &bytes, &origins)?;
        check_headers(&destination, &index, &origins)?;
        streams.push(fs::read(base.join("audio.jsonl"))?);
    }
    assert_eq!(
        streams.first(),
        streams.get(1),
        "audio records are deterministic"
    );
    Ok(())
}

#[test]
fn an_empty_animation_definition_selection_is_complete() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = archive(folder.path(), &wem_bytes())?;
    let destination = folder.path().join("audio-control");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &destination, &options)?)?;
    assert!(support::records(&destination, &index, "animation-definition.jsonl")?.is_empty());
    Ok(())
}

fn check_original_bytes(bytes: &[u8]) -> TestResult {
    let parsed = Wem::parse(bytes)?;
    let first = parsed.chunk(*b"fmt ").ok_or("first fmt body")?;
    assert_eq!(
        first.get(2..4),
        Some([1, 0].as_slice()),
        "first fmt is mono"
    );
    assert_eq!(
        bytes.get(46..48),
        Some([2, 0].as_slice()),
        "second fmt is stereo"
    );
    assert_eq!(
        bytes.get(60..68),
        Some(b"data\x08\0\0\0".as_slice()),
        "first DATA extent"
    );
    assert_eq!(
        bytes.get(76..80),
        Some(b"fmt ".as_slice()),
        "post-data fmt decoy"
    );
    assert_eq!(
        bytes.get(100..108),
        Some(b"data\x20\0\0\0".as_slice()),
        "post-data DATA decoy"
    );
    assert_eq!(parsed.channels(), 2, "last pre-data fmt supplies channels");
    assert_eq!(
        parsed.sample_count(),
        Some(2),
        "first DATA supplies frame count"
    );
    Ok(())
}

fn archive(root: &Path, bytes: &[u8]) -> TestResult<Archive> {
    let path = root.join("Data.p4k");
    let nested = TestContainer::new().entry(TestEntry::stored(CHILD, bytes));
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SAME, bytes))
        .entry(TestEntry::zstd(SAME, bytes).encrypted())
        .entry(TestEntry::stored("Audio/excluded.wem", bytes))
        .entry(TestEntry::stored(CONTAINER, nested.build()).encrypted())
        .entry(TestEntry::stored("unmatched.bin", b"unrelated"))
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let container = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == CONTAINER)
        .ok_or("actual encrypted containing Entry")?;
    let inner = archive.open_container(container)?;
    let child = inner.entries().first().ok_or("original nested WEM")?;
    let observed = inner
        .observed_entry(child)
        .ok_or("observed original nested WEM")?;
    assert_eq!(
        child.path(),
        CHILD,
        "the nested directory's original leaf path"
    );
    assert_eq!(
        observed.stored_encryption(),
        SourceEncryption::Unknown,
        "an inner standard header without an authoritative CIG declaration remains unknown",
    );
    Ok(archive)
}

fn check_selection(index: &Index) -> TestResult {
    assert!(
        index.complete && index.enumeration_complete,
        "complete selected generation"
    );
    assert_eq!(
        (
            index.source_entries.enumerated,
            index.source_entries.written,
            index.source_entries.skipped
        ),
        (6, 3, 3),
        "both duplicate Entries and the child are selected; exclusion and unmatched parent remain distinct"
    );
    let [audio] = index.kinds.as_slice() else {
        return Err("only the explicitly selected Audio provider".into());
    };
    assert_eq!(
        audio.provider.kind,
        MediaKind::Audio,
        "real Audio registration"
    );
    assert_eq!(
        audio.counts.source_entries.written, 3,
        "one written callback per selected WEM"
    );
    assert_eq!(
        audio.counts.converted_outputs.written, 0,
        "retained WEM is a raw artifact"
    );
    Ok(())
}

const OWNERS: [Owner; 3] = [
    Owner {
        path: SAME,
        entry: 0,
        parent: None,
        stored: "unencrypted",
        metadata: false,
        payload: false,
    },
    Owner {
        path: SAME,
        entry: 1,
        parent: None,
        stored: "encrypted",
        metadata: false,
        payload: true,
    },
    Owner {
        path: CHILD,
        entry: 0,
        parent: Some(3),
        stored: "unknown",
        metadata: true,
        payload: true,
    },
];

fn check_raw(base: &Path, index: &Index, bytes: &[u8], origins: &Origins) -> TestResult {
    let sources: Vec<Source> = read_rows(base.join(&index.source_manifest.path))?;
    assert_eq!(
        sources.len(),
        OWNERS.len(),
        "one source-manifest row per retained WEM"
    );
    let digest = Checksum::Sha256(Sha256::digest(bytes).into()).hex();
    let raw_path = format!("raw/{digest}");
    assert_eq!(
        fs::read(base.join(&raw_path))?.as_slice(),
        bytes,
        "usable verified original WEM bytes"
    );
    for (source, owner) in sources.iter().zip(OWNERS) {
        check_identity(source, owner)?;
        let raw = source.raw.as_ref().ok_or("actual retained raw WEM")?;
        assert_eq!(raw.path, raw_path, "equal bytes reuse one content address");
        assert_eq!(
            raw.size,
            u64::try_from(bytes.len())?,
            "raw receipt actual length"
        );
        assert_eq!(
            raw.digest.hex, digest,
            "digest measures the actual retained bytes"
        );
        assert!(
            matches!(raw.digest.scope, ByteScope::DecodedEntryPayload),
            "raw hash byte scope"
        );
    }
    let artifacts: Vec<Artifact> = read_rows(base.join(&index.artifact_manifest.path))?;
    let mut matching = artifacts
        .iter()
        .enumerate()
        .filter(|(_, artifact)| artifact.path == raw_path);
    for owner in OWNERS {
        let (ordinal, artifact) = matching.next().ok_or("original raw artifact occurrence")?;
        assert_eq!(
            artifact.size,
            u64::try_from(bytes.len())?,
            "artifact actual length"
        );
        assert_eq!(artifact.digest.hex, digest, "artifact manifest digest");
        let receipt = origins.field("artifacts", ordinal, "/digest/hex", owner.payload)?;
        origins.require_owner(receipt, owner)?;
    }
    assert!(
        matching.next().is_none(),
        "deduplication preserves exactly three artifact occurrences"
    );
    Ok(())
}

fn check_headers(destination: &Path, index: &Index, origins: &Origins) -> TestResult {
    let records = support::records(destination, index, "audio.jsonl")?;
    chunks::check(&records, origins)?;
    let mut headers = records
        .iter()
        .enumerate()
        .filter(|(_, record)| record.subresource.namespace == WEM_HEADER);
    for owner in OWNERS {
        let (ordinal, record) = headers.next().ok_or("typed WEM header")?;
        assert_eq!(
            record.kind,
            MediaKind::Audio,
            "Audio stream owns WEM header"
        );
        assert_eq!(
            record.status,
            Status::Complete,
            "supported WEM header is complete"
        );
        assert!(
            record.subresource.indices.is_empty(),
            "one WEM header in each physical source"
        );
        check_identity(&record.source, owner)?;
        assert_eq!(
            record.data.get("record_type"),
            Some(&json!("wem")),
            "typed WEM payload"
        );
        for (field, expected) in [
            ("format_tag", json!(1)),
            ("channels", json!(2)),
            ("sample_rate", json!(48_000)),
            ("sample_count", json!("2")),
        ] {
            assert_eq!(
                record.data.pointer(&format!("/payload/{field}")),
                Some(&expected),
                "original WEM {field}"
            );
        }
        let name = origins.field(
            "records",
            ordinal,
            "/source/identity/entry/path",
            owner.metadata,
        )?;
        origins.require_owner(name, owner)?;
        check_uses(origins, ordinal, owner)?;
    }
    assert!(
        headers.next().is_none(),
        "only the three selected physical WEM headers are emitted"
    );
    Ok(())
}

fn check_uses(origins: &Origins, ordinal: usize, owner: Owner) -> TestResult {
    for (pointer, role, body, field) in [
        ("/data/payload/channels", "payload", FORMAT_BODY, "channels"),
        (
            "/data/payload/sample_count",
            "conversion",
            DATA_BODY,
            "bodyLength",
        ),
    ] {
        let receipt = origins.field("records", ordinal, pointer, owner.payload)?;
        let uses = origins.require_owner(receipt, owner)?;
        assert!(
            uses.iter().any(|used| {
                used.get("scope") == Some(&json!("payload"))
                    && used.get("role") == Some(&json!(role))
                    && used.get("location") == Some(&json!({
                        "type":"subresource", "namespace":"wemChunk", "indices":[body], "field":field
                    }))
            }),
            "{pointer} retains its exact pre-data or first-data original occurrence"
        );
    }
    Ok(())
}
