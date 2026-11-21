// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions, Soundbank};
use p4kx_media::{
    ExportOptions, ExportOutcome, Identity, MediaError, MediaKind, MediaRecord, Status,
};
use p4kx_testkit::{
    ArchiveBuilder, TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase, test_key,
};
use serde_json::{Value, json};

use super::audio_bank_hirc_action_origins::{self as origins, Origins};
use super::{layout_health, support};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SAME_NAME: &str = "containers.BNK";
const CONTAINER: &str = "audio-bank-hirc-container";
const CHILD: &str = "audio-bank-hirc-container-child";
const STREAM: &str = "audio.jsonl";
const IDS: [u32; 3] = [0x9814_5678, 0x9814_5678, 0x3255_9187];
const PARENT: u32 = 0x2C21_1B2E;
const OBJECT_ID: u32 = 67;

#[test]
fn container_export_preserves_original_parent_mode_and_ordered_duplicate_children() -> TestResult {
    check_container_pair(TestContainerKind::Random, TestContainerKind::Sequence)
}

#[test]
fn switch_and_blend_export_a_null_mode_with_the_original_layout_selectors() -> TestResult {
    check_container_pair(TestContainerKind::Switch, TestContainerKind::Blend)
}

fn check_container_pair(first: TestContainerKind, second: TestContainerKind) -> TestResult {
    let folder = tempfile::tempdir()?;
    let first_bytes = bank(first, TestNodeBase::Plain);
    let second_bytes = bank(second, TestNodeBase::Full);
    let archive = archive(folder.path(), &first_bytes, &second_bytes)?;
    let out = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &out, &options())?)?;
    let base = out.join(&index.artifact_base);
    let rows = support::records(&out, &index, STREAM)?;
    let evidence = Origins::read(&base)?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for (entry, (variant, bytes)) in [(first, &first_bytes), (second, &second_bytes)]
        .into_iter()
        .enumerate()
    {
        let (kind, mode) = match variant {
            TestContainerKind::Random => ("random", Some(0)),
            TestContainerKind::Sequence => ("sequence", Some(1)),
            TestContainerKind::Switch => ("switch", None),
            TestContainerKind::Blend => ("blend", None),
        };
        let (ordinal, row) = find(&rows, entry, CONTAINER, &[1, 0])?;
        check_header(row, kind, mode);
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "the exact typed row satisfies the generated media schema"
        );
        let raw = row.source.raw.as_ref().ok_or("retained original bank")?;
        assert_eq!(
            &fs::read(base.join(&raw.path))?,
            bytes,
            "the typed container retains the original bank bytes"
        );
        let parent = unique(bytes, &PARENT.to_le_bytes())?;
        exact(
            &evidence,
            ordinal,
            entry,
            "parent_id",
            "containerParentId",
            &parent,
        )?;
        let child_words: Vec<_> = IDS.into_iter().flat_map(u32::to_le_bytes).collect();
        let children = unique(bytes, &child_words)?;
        check_mode(&evidence, ordinal, entry, mode, children.start)?;
        check_children(&rows, &evidence, &schema, entry, children.start)?;
    }
    Ok(())
}

fn check_children(
    rows: &[MediaRecord],
    evidence: &Origins,
    schema: &jsonschema::Validator,
    entry: usize,
    children_start: usize,
) -> TestResult {
    for (index, id) in IDS.into_iter().enumerate() {
        let (ordinal, row) = find(rows, entry, CHILD, &[1, 0, u64::try_from(index)?])?;
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "the exact typed row satisfies the generated media schema"
        );
        assert_eq!(
            row.data.pointer("/payload/child_id"),
            Some(&json!(id)),
            "each ordinal keeps its original child ID"
        );
        let start = children_start + index * 4;
        assert_eq!(
            row.data.pointer("/payload/id_range"),
            Some(&range(&(start..start + 4))),
            "the child row addresses the original word"
        );
        exact(
            evidence,
            ordinal,
            entry,
            "child_id",
            "containerChildId",
            &(start..start + 4),
        )?;
    }
    Ok(())
}

fn check_mode(
    evidence: &Origins,
    row: usize,
    entry: usize,
    mode: Option<u8>,
    children_start: usize,
) -> TestResult {
    if mode.is_some() {
        let offset = children_start
            .checked_sub(6)
            .ok_or("original mode before count")?;
        return exact(
            evidence,
            row,
            entry,
            "mode_code",
            "containerMode",
            &(offset..offset + 1),
        );
    }
    for field in ["/data/payload/mode_code", "/data/payload/container_kind"] {
        let uses = evidence.uses(row, entry, field)?;
        for selector in ["typeCode", "version"] {
            assert!(
                uses.iter().any(|used| {
                    used.get("role").and_then(Value::as_str) == Some("selector")
                        && used.pointer("/location/field").and_then(Value::as_str) == Some(selector)
                }),
                "{field} uses the original {selector}"
            );
        }
        assert!(
            !origins::has_field(&uses, "containerMode"),
            "{field} has no authored mode byte"
        );
    }
    Ok(())
}

fn check_header(row: &MediaRecord, kind: &str, mode: Option<u8>) {
    assert_eq!(
        row.status,
        Status::Complete,
        "the valid fixture projects without a coverage failure"
    );
    assert_eq!(
        row.data.pointer("/payload/container_kind"),
        Some(&json!(kind)),
        "the layout preserves its decoded container kind"
    );
    assert_eq!(
        row.data.pointer("/payload/mode_code"),
        Some(&json!(mode)),
        "the mode reflects whether this layout stores a mode word"
    );
    assert_eq!(
        row.data.pointer("/payload/parent_id"),
        Some(&json!(PARENT)),
        "the original parent word is retained"
    );
    assert_eq!(
        row.data.pointer("/payload/child_count"),
        Some(&json!(IDS.len())),
        "the declared count includes repeated children"
    );
}

#[test]
fn malformed_container_suffix_keeps_admitted_children_in_diagnostic_output() -> TestResult {
    let folder = tempfile::tempdir()?;
    let valid = bank(TestContainerKind::Blend, TestNodeBase::Full);
    let read = Soundbank::parse(&valid)?;
    let body = read.object_body(read.objects().first().ok_or("original blend")?);
    let body = body.get(4..body.len() - 1).ok_or("omit final flag")?;
    let malformed = TestBank::new(1)
        .object(TestHircObject::raw(9, OBJECT_ID, body))
        .bytes();
    let archive = archive(folder.path(), &malformed, &valid)?;
    let out = folder.path().join("out");
    let failure = p4kx_media::export(&archive, &out, &options())
        .err()
        .ok_or("known suffix corruption must refuse normal publication")?;
    assert!(matches!(
        layout_health::leaf(&failure),
        MediaError::ProjectionLoss {
            kind: MediaKind::Audio,
            ..
        }
    ));
    let options = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &out, &options)?
    else {
        return Err("retained diagnostic generation".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic generation path")?;
    let rows = support::records(base, &index, STREAM)?;
    let (_, container) = find(&rows, 0, CONTAINER, &[1, 0])?;
    assert_eq!(container.status, Status::Partial);
    assert_eq!(
        container.data.pointer("/payload/layout"),
        Some(&json!("malformed"))
    );
    let (_, child) = find(&rows, 0, CHILD, &[1, 0, 2])?;
    let id = IDS.last().ok_or("last original fixture child")?;
    assert_eq!(child.data.pointer("/payload/child_id"), Some(&json!(id)));
    assert_eq!(child.status, Status::Complete);
    Ok(())
}

fn find<'rows>(
    rows: &'rows [MediaRecord],
    entry: usize,
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'rows MediaRecord)> {
    rows.iter().enumerate().find(|(_, row)| {
        row.subresource.namespace == namespace && row.subresource.indices == indices
            && matches!(&row.source.identity, Identity::Entry { entry: owner, .. } if owner.entry_index == entry)
    }).ok_or_else(|| "required physical container occurrence".into())
}

fn exact(
    evidence: &Origins,
    row: usize,
    entry: usize,
    field: &str,
    source: &str,
    range: &Range<usize>,
) -> TestResult {
    let uses = evidence.uses(row, entry, &format!("/data/payload/{field}"))?;
    assert!(
        origins::has_exact(&uses, source, "payload", range),
        "exact original {field} range"
    );
    Ok(())
}

fn unique(bytes: &[u8], pattern: &[u8]) -> TestResult<Range<usize>> {
    let mut found = bytes
        .windows(pattern.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == pattern).then_some(offset));
    let start = found.next().ok_or("original fixture byte pattern")?;
    assert!(
        found.next().is_none(),
        "independent fixture witness is unique"
    );
    Ok(start..start + pattern.len())
}

fn range(extent: &Range<usize>) -> Value {
    json!({"basis":{"type":"entry"},"start":extent.start.to_string(),"end_exclusive":extent.end.to_string()})
}

fn archive(directory: &Path, first: &[u8], second: &[u8]) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(SAME_NAME, first))
        .entry(TestEntry::zstd(SAME_NAME, second).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn bank(kind: TestContainerKind, base: TestNodeBase) -> Vec<u8> {
    TestBank::new(1)
        .object(TestHircObject::container(kind, OBJECT_ID, &IDS, base))
        .bytes()
}

fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    }
}
