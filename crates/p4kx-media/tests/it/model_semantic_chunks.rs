// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry, TestModel, test_key};
use serde_json::{Value, json};

use super::model_source_provider::field_uses;
use super::support::TestResult;

#[test]
fn semantic_record_references_own_each_supported_and_failed_chunk_without_reclassifying_raw_rows()
-> TestResult {
    let temp = tempfile::tempdir()?;
    let file = original_chunks();
    let archive = archive(temp.path(), &file)?;
    let output = temp.path().join("semantic");
    let selection = ExportOptions {
        kinds: vec![MediaKind::Model],
        diagnostic: true,
        ..ExportOptions::default()
    };
    let ExportOutcome::Incomplete { diagnostic, .. } =
        p4kx_media::export(&archive, &output, &selection)?
    else {
        return Err("a malformed typed chunk requires a diagnostic generation".into());
    };
    let generation = diagnostic.parent().ok_or("diagnostic generation")?;
    let rows = fs::read_to_string(generation.join("model.jsonl"))?
        .lines()
        .map(serde_json::from_str::<MediaRecord>)
        .collect::<Result<Vec<_>, _>>()?;
    let source = ChunkFile::parse(file.as_slice())?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "closed model record schema"
        );
    }
    for owner in [0, 1] {
        let model_rows = rows.iter().enumerate().filter(|(_, row)| {
            matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.entry_index == owner)
        }).collect::<Vec<_>>();
        for (ordinal, reference) in [
            None,
            None,
            Some(("model-lod-distance-words", "words", Status::Complete)),
            Some((
                "model-lod-distance-words",
                "unmeasured-extent",
                Status::Complete,
            )),
            Some(("model-node-table", "measured", Status::Complete)),
            Some(("model-node-table", "unsupported-version", Status::Complete)),
            Some(("model-node-table", "malformed", Status::Partial)),
        ]
        .into_iter()
        .enumerate()
        {
            assert_semantic_reference(&model_rows, ordinal, reference)?;
        }
        assert_link_origin(generation, &source, &model_rows, owner)?;
    }
    Ok(())
}

fn original_chunks() -> Vec<u8> {
    TestChunkFile::ivo()
        .chunk(ivo::PHYSICAL_HIERARCHY.id(), 0x900, [0; 16])
        .chunk(0x0BAD_BEEF, 0x900, [0; 16])
        .chunk(ivo::LOD_DISTANCES.id(), 0x900, [0; 48])
        .chunk(ivo::LOD_DISTANCES.id(), 0x900, [0; 32])
        .chunk(
            ivo::NODE_MESH_COMBO.id(),
            0x900,
            TestModel::cube().nmc_chunk(),
        )
        .chunk(ivo::NODE_MESH_COMBO.id(), 0xc01, [0; 16])
        .chunk(ivo::NODE_MESH_COMBO.id(), 0x900, [0; 16])
        .bytes()
}

fn assert_link_origin(
    generation: &Path,
    source: &ChunkFile<&[u8]>,
    model_rows: &[(usize, &MediaRecord)],
    owner: usize,
) -> TestResult {
    let (directory_row, _) = model_rows
        .iter()
        .find(|(_, row)| {
            row.subresource.namespace == "model-chunk" && row.subresource.indices == [2]
        })
        .ok_or("typed directory row for field provenance")?;
    let original = source
        .table_row(2)
        .ok_or("type word of typed original chunk")?;
    let uses = field_uses(
        generation,
        *directory_row,
        "/data/payload/semantic_record",
        owner,
        owner == 1,
    )?;
    assert_eq!(uses.len(), 1, "one exact physical type-word use");
    let expected = json!({
        "type": "subresource",
        "namespace": "model-byte-range",
        "indices": [original.kind.start.to_string(), original.kind.end.to_string()],
        "field": "modelChunkType"
    });
    assert_eq!(
        uses.first().and_then(|used| used.get("location")),
        Some(&expected),
        "original type-word byte interval"
    );
    Ok(())
}

fn assert_semantic_reference(
    rows: &[(usize, &MediaRecord)],
    ordinal: usize,
    reference: Option<(&str, &str, Status)>,
) -> TestResult {
    let index = [ordinal as u64];
    let (_, directory) = rows
        .iter()
        .find(|(_, row)| {
            row.subresource.namespace == "model-chunk" && row.subresource.indices == index
        })
        .ok_or("physical directory row")?;
    let projection = if ordinal == 1 {
        "unknown-type"
    } else {
        "known-opaque"
    };
    assert_eq!(
        directory.data.pointer("/payload/projection"),
        Some(&json!(projection)),
        "legacy classification is unchanged"
    );
    let linked = directory
        .data
        .pointer("/payload/semantic_record")
        .and_then(Value::as_str);
    let Some((namespace, disposition, status)) = reference else {
        assert_eq!(linked, None, "raw-only chunk has no semantic owner");
        return Ok(());
    };
    assert_eq!(
        linked,
        Some(namespace),
        "link identifies the matching positioned disposition"
    );
    let mut targets = rows.iter().filter(|(_, row)| {
        row.subresource.namespace == namespace && row.subresource.indices == index
    });
    let (_, semantic) = targets.next().ok_or("linked semantic row")?;
    assert!(
        targets.next().is_none(),
        "one semantic row per positioned owner"
    );
    assert_eq!(
        semantic.data.pointer("/payload/disposition"),
        Some(&json!(disposition)),
        "actual semantic disposition"
    );
    assert_eq!(semantic.status, status, "actual semantic health");
    assert_eq!(
        semantic.source.raw.as_ref(),
        directory.source.raw.as_ref(),
        "one shared raw original"
    );
    Ok(())
}

fn archive(folder: &Path, bytes: &[u8]) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Synthetic/source.CGF", bytes))
        .entry(TestEntry::zstd("Synthetic/source.CGF", bytes).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}
