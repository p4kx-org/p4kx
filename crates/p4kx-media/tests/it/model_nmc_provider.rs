// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry, TestModel, TestNode, test_key};
use serde_json::{Value, json};

use super::model_source_provider::field_uses;
use super::support;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const MODEL: &str = "model.jsonl";

#[test]
fn helper_and_property_rows_retain_original_words_and_encrypted_ranges() -> TestResult {
    let temp = tempfile::tempdir()?;
    let (payload, property_start) = nmc()?;
    let file = TestChunkFile::ivo()
        .chunk(ivo::NODE_MESH_COMBO.id(), 0x900, payload)
        .chunk(ivo::NODE_MESH_COMBO.id(), 0x901, b"opaque")
        .bytes();
    let directory = ChunkFile::parse(file.as_slice())?;
    let chunk = directory.chunks().first().ok_or("original NMC chunk")?;
    let output = temp.path().join("out");
    let archive = archive(temp.path(), &file)?;
    let index = support::published(p4kx_media::export(&archive, &output, &options(false))?)?;
    let root = output.join(&index.artifact_base);
    let rows = support::records(&output, &index, MODEL)?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "closed model row"
        );
    }
    let (_, table) = row(&rows, "model-node-table", &[0])?;
    field(table, "nodes", &json!(2));
    field(
        row(&rows, "model-node-table", &[1])?.1,
        "disposition",
        &json!("unsupported-version"),
    );
    let retained = table.source.raw.as_ref().ok_or("retained model payload")?;
    assert_eq!(
        fs::read(root.join(&retained.path))?,
        file,
        "retained model bytes"
    );
    let (ordinal, helper) = row(&rows, "model-node", &[0, 0])?;
    let position = chunk.offset() + 32;
    assert_helper(helper, &root, ordinal, position)?;
    assert_properties(&rows, &root, chunk.offset() + property_start)?;
    let (index_row, material) = row(&rows, "model-node-material-index", &[0, 0])?;
    let material_start = position + 2 * 208 + 32;
    field(material, "word", &json!(1));
    assert_exact(&root, index_row, "word", material_start..material_start + 2)?;
    Ok(())
}

fn assert_helper(row: &MediaRecord, root: &Path, ordinal: usize, start: u64) -> TestResult {
    for (path, value) in [
        ("stored_id", json!(1)),
        ("geometry_type", json!(2)),
        ("helper", json!(true)),
        ("parent_word", json!(u16::MAX)),
        ("local_matrix_bits/0/3", json!((-0.0_f32).to_bits())),
        ("companion_matrix_bits/1/3", json!(4_f32.to_bits())),
        (
            "scale_bits",
            json!([2_f32.to_bits(), 3_f32.to_bits(), 4_f32.to_bits()]),
        ),
        ("name/utf8", json!("helper\nsecond name line")),
        ("name/display_name_utf8", json!("helper")),
        ("property_entry_terminated", json!(true)),
    ] {
        field(row, path, &value);
    }
    assert_exact(root, ordinal, "helper", start + 150..start + 152)?;
    assert_exact(
        root,
        ordinal,
        "local_matrix_bits/0/3",
        start + 92..start + 96,
    )?;
    assert_exact(root, ordinal, "scale_bits/1", start + 132..start + 136)?;
    Ok(())
}

fn assert_properties(rows: &[MediaRecord], root: &Path, start: u64) -> TestResult {
    let (_, child) = row(rows, "model-node", &[0, 1])?;
    field(child, "parent_word", &json!(0));
    field(child, "geometry_type", &json!(9));
    field(child, "helper", &json!(false));
    field(child, "property_entry_terminated", &json!(true));
    assert_eq!(
        child.data.pointer("/payload/property_entry/start"),
        child.data.pointer("/payload/property_entry/end_exclusive"),
        "empty authored slot has a zero-length range"
    );
    for (line, raw, assignment) in [
        (0, json!("kind=one\r"), json!({"key":"kind","value":"one"})),
        (1, json!("raw comment\r"), Value::Null),
        (2, json!("kind=two\r"), json!({"key":"kind","value":"two"})),
        (3, Value::Null, json!({"key":"bad","value":null})),
        (4, json!("empty=\r"), json!({"key":"empty","value":""})),
    ] {
        let (_, record) = row(rows, "model-node-property", &[0, 0, line])?;
        field(record, "raw_utf8", &raw);
        field(record, "assignment", &assignment);
    }
    let (ordinal, _) = row(rows, "model-node-property", &[0, 0, 0])?;
    assert_exact(root, ordinal, "assignment/key", start..start + 4)?;
    assert_exact(root, ordinal, "assignment/value", start + 5..start + 8)?;
    assert!(
        row(rows, "model-node-property", &[0, 0, 5]).is_err(),
        "no extra line after final separator"
    );
    assert!(
        row(rows, "model-node-property", &[0, 1, 0]).is_err(),
        "no property lines in an empty slot"
    );
    Ok(())
}

#[test]
fn truncated_property_extent_is_diagnostic_with_readable_siblings() -> TestResult {
    let temp = tempfile::tempdir()?;
    let (mut payload, _) = nmc()?;
    payload.truncate(payload.len() - 2);
    let bytes = TestChunkFile::ivo()
        .chunk(ivo::NODE_MESH_COMBO.id(), 0x900, payload)
        .bytes();
    let archive = archive(temp.path(), &bytes)?;
    let output = temp.path().join("out");
    let ExportOutcome::Incomplete { diagnostic, .. } =
        p4kx_media::export(&archive, &output, &options(true))?
    else {
        return Err("truncated stored property extent requires partial diagnostic".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic generation")?;
    let rows = fs::read_to_string(base.join(MODEL))?
        .lines()
        .map(serde_json::from_str::<MediaRecord>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        row(&rows, "model-node-table", &[0])?.1.status,
        Status::Partial,
        "truncated source extent"
    );
    field(
        row(&rows, "model-node", &[0, 0])?.1,
        "property_entry_terminated",
        &json!(false),
    );
    field(
        row(&rows, "model-node", &[0, 1])?.1,
        "property_entry",
        &Value::Null,
    );
    assert_eq!(
        row(&rows, "model-node-property", &[0, 0, 0])?.1.status,
        Status::Complete,
        "readable sibling line"
    );
    assert!(
        !output.join("media-index.json").exists(),
        "partial capture preserves the previous published index"
    );
    Ok(())
}

fn nmc() -> TestResult<(Vec<u8>, u64)> {
    let local = [
        [1.0, 0.0, 0.0, -0.0],
        [0.0, 1.0, 0.0, 4.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    let original = TestModel::cube()
        .helpers_only()
        .with_nodes(vec![
            TestNode {
                name: "helper\nsecond name line".into(),
                parent: None,
                local,
                kind: 2,
            },
            TestNode {
                name: "child".into(),
                parent: Some(0),
                local,
                kind: 9,
            },
        ])
        .with_node_details(0, [2.0, 3.0, 4.0], &[("original", "marker")])
        .nmc_chunk();
    let marker = b"original=marker\r\n";
    let start = original
        .windows(marker.len())
        .position(|window| window == marker)
        .ok_or("physical property marker")?;
    let mut nmc = original.get(..start).ok_or("fixture prefix")?.to_vec();
    let properties = b"kind=one\r\nraw comment\r\nkind=two\r\nbad=\xFF\r\nempty=\r\n\0\0";
    let length = u32::try_from(properties.len())?;
    nmc.get_mut(24..28)
        .ok_or("original property length word")?
        .copy_from_slice(&length.to_le_bytes());
    nmc.extend_from_slice(properties);
    Ok((nmc, start as u64))
}

fn row<'a>(
    rows: &'a [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'a MediaRecord)> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == namespace && row.subresource.indices == indices
        })
        .ok_or_else(|| format!("missing {namespace} {indices:?}").into())
}

fn field(row: &MediaRecord, field: &str, expected: &Value) {
    assert_eq!(
        row.data.pointer(&format!("/payload/{field}")),
        Some(expected),
        "original {field}"
    );
}

fn assert_exact(root: &Path, ordinal: usize, field: &str, range: Range<u64>) -> TestResult {
    let uses = field_uses(root, ordinal, &format!("/data/payload/{field}"), 0, true)?;
    let expected = json!({
        "type": "subresource",
        "namespace": "model-byte-range",
        "indices": [range.start.to_string(), range.end.to_string()],
        "field": field,
    });
    assert!(
        uses.iter()
            .any(|usage| usage.get("location") == Some(&expected)),
        "original {field} bytes"
    );
    Ok(())
}

fn archive(path: &Path, bytes: &[u8]) -> TestResult<Archive> {
    let path = path.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Synthetic/helper.cgf", bytes).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn options(diagnostic: bool) -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Model],
        diagnostic,
        ..ExportOptions::default()
    }
}
