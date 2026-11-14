// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::ivo;
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry, TestSkeleton, test_key};
use serde_json::{Value, json};

use super::model_source_provider::field_uses;
use super::support::{self, TestResult};

#[test]
fn model_skeletons_keep_physical_chunk_and_bone_coordinates() -> TestResult {
    let folder = tempfile::tempdir()?;
    let skeleton = TestSkeleton::two_bones();
    let file = TestChunkFile::ivo()
        .chunk(
            ivo::COMPILED_BONES_2.id(),
            0x900,
            skeleton.compiled_bones_2(),
        )
        .chunk(ivo::COMPILED_BONES.id(), 0x901, skeleton.compiled_bones())
        .chunk(ivo::COMPILED_BONES_2.id(), 0x901, [0; 4]);
    let offsets = file.offsets();
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("first.skin", file.bytes()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with(&path, OpenOptions::new().key(Some(test_key())))?;
    let out = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Model],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &out, &options)?)?;
    let rows = support::records(&out, &index, "model.jsonl")?;
    let bone_row = assert_rows(&rows)?;
    let original = offsets.first().copied().ok_or("physical skeleton offset")?;
    let location = json!([original + 84, original + 100].map(|word| word.to_string()));
    let uses = field_uses(
        &out.join(&index.artifact_base),
        bone_row,
        "/data/payload/relative/rotation_xyzw",
        0,
        true,
    )?;
    assert!(
        uses.iter()
            .any(|fact| fact.pointer("/location/indices") == Some(&location)),
        "rotation refers to its exact original word span"
    );
    Ok(())
}

fn assert_rows(rows: &[MediaRecord]) -> Result<usize, Box<dyn std::error::Error>> {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        assert!(
            validator.is_valid(&serde_json::to_value(row)?),
            "closed skeleton wire schema"
        );
    }
    let (bone_row, tip) = row(rows, "model-skeleton-bone", &[0, 1])?;
    assert_eq!(
        tip.data.pointer("/payload/name"),
        Some(&json!("tip")),
        "physical bone name"
    );
    assert_eq!(
        tip.data.pointer("/payload/parent"),
        Some(&json!(0)),
        "physical parent ordinal"
    );
    assert_eq!(
        tip.data.pointer("/payload/node"),
        Some(&Value::Null),
        "expanded format has no driven-node word"
    );
    assert_eq!(
        tip.data.pointer("/payload/relative/rotation_xyzw"),
        Some(&json!([0.5, 0.5, 0.5, 0.5])),
        "stored XYZW quaternion",
    );
    let compact = row(rows, "model-skeleton-bone", &[1, 1])?.1;
    assert_eq!(
        compact.data.pointer("/payload/node"),
        Some(&json!(1)),
        "compact source node is retained"
    );
    let newer = row(rows, "model-skeleton-chunk", &[2])?.1;
    assert_eq!(
        newer.data.pointer("/payload/disposition"),
        Some(&json!("unsupported-version")),
        "future layout retains its disposition"
    );
    assert_eq!(
        newer.data.pointer("/payload/bone_count"),
        Some(&Value::Null),
        "future layout has no guessed count"
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.subresource.namespace == "model-skeleton-bone")
            .count(),
        4,
        "one row for each measured original bone",
    );
    Ok(bone_row)
}

fn row<'a>(
    rows: &'a [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> Result<(usize, &'a MediaRecord), Box<dyn std::error::Error>> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == namespace && row.subresource.indices == indices
        })
        .ok_or_else(|| format!("missing {namespace} {indices:?}").into())
}
