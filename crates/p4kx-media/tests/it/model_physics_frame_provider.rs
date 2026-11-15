// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord, Status};
use p4kx_testkit::{TestChunkFile, TestEntry};
use serde_json::{Value, json};

use super::model_physics_provider::{collision_shape, physics_table};
use super::model_source_provider::{assert_exact_use, field_uses};
use super::support::{self, TestResult};

const HEADER: usize = 32;
const STRIDE: usize = 208;
const FOOTER: usize = 32;
const MATRIX_AT: usize = 80;
const SCALE_AT: usize = 128;
const PARENT_AT: usize = 148;

pub(super) const TWO_ANCESTOR_ENTITY_FRAME: [[f64; 4]; 3] = [
    [0.0, -15.0, 0.0, 51.0],
    [0.0, 0.0, -12.0, -92.0],
    [4.0, 0.0, 0.0, 59.0],
];

#[test]
fn collision_frame_composes_two_nonuniform_ancestors_and_tracks_physical_fields() -> TestResult {
    let folder = tempfile::tempdir()?;
    let file = nmc_collision_fixture(false)?;
    let chunks = ChunkFile::parse(file.as_slice())?;
    let node_offset = chunks.chunks().get(1).ok_or("physical NMC chunk")?.offset();
    let archive = support::archive_with(folder.path(), [TestEntry::stored("source.cgf", file)])?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Model],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "model.jsonl")?;
    verify_rows(&rows)?;
    let (ordinal, _) = find(&rows, "model-physics-frame", &[0, 0])?;
    let base = output.join(&index.artifact_base);
    let evidence = field_uses(&base, ordinal, "/data/payload/entity", 0, false)?;
    verify_ancestor_origins(&evidence, node_offset)?;
    Ok(())
}

fn verify_ancestor_origins(evidence: &[Value], node_offset: u64) -> TestResult {
    for (field, start, len) in [
        ("modelNmcLocalMatrix", MATRIX_AT, 48),
        ("modelNmcScale", SCALE_AT, 12),
        ("modelNmcParent", PARENT_AT, 2),
    ] {
        let actual = evidence
            .iter()
            .filter(|value| value.pointer("/location/field").and_then(Value::as_str) == Some(field))
            .count();
        assert_eq!(
            actual, 3,
            "only the three physical chain members supply {field}"
        );
        for row in [0, 2, 3] {
            let offset = node_offset + u64::try_from(HEADER + row * STRIDE + start)?;
            assert_exact_use(evidence, field, offset..offset + len);
        }
    }
    Ok(())
}

#[test]
fn a_missing_node_footer_keeps_the_observed_physics_frame_in_diagnostics() -> TestResult {
    let folder = tempfile::tempdir()?;
    let file = nmc_collision_fixture(true)?;
    let archive = support::archive_with(folder.path(), [TestEntry::stored("source.cgf", file)])?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Model],
        diagnostic: true,
        ..ExportOptions::default()
    };
    let ExportOutcome::Incomplete { diagnostic, .. } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("missing NMC footer must mark the table partial".into());
    };
    let root = diagnostic.parent().ok_or("diagnostic generation")?;
    let rows = std::fs::read_to_string(root.join("model.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<MediaRecord>, _>>()?;
    verify_rows(&rows)?;
    let (_, table) = find(&rows, "model-node-table", &[1])?;
    assert_eq!(
        table.status,
        Status::Partial,
        "original table remains partial"
    );
    assert_eq!(
        table.data.pointer("/payload/disposition"),
        Some(&json!("malformed")),
        "explicit disposition of the missing second portion"
    );
    Ok(())
}

fn verify_rows(rows: &[MediaRecord]) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "NMC sidecars satisfy the built-in schema"
        );
    }
    let (_, helper) = find(rows, "model-physics-node", &[1, 0])?;
    assert_eq!(
        helper.data.pointer("/payload/stored_id"),
        Some(&json!(912)),
        "raw sparse ID"
    );
    assert_eq!(
        helper.data.pointer("/payload/geometry_type"),
        Some(&json!(3)),
        "physical helper is retained"
    );
    let (_, frame) = find(rows, "model-physics-frame", &[0, 0])?;
    let entity = json!(TWO_ANCESTOR_ENTITY_FRAME);
    assert_eq!(
        frame.data.pointer("/payload/entity"),
        Some(&entity),
        "affine composes the source helper and both rotated, scaled ancestors"
    );
    assert_eq!(
        frame.data.pointer("/payload/node_chunk"),
        Some(&json!(1)),
        "physical node table owner"
    );
    for (index, disposition) in [(1, "cycle"), (2, "whole-object"), (3, "missing-node")] {
        let (_, frame) = find(rows, "model-physics-frame", &[0, index])?;
        assert_eq!(
            frame.data.pointer("/payload/disposition"),
            Some(&json!(disposition)),
            "unresolved node disposition"
        );
        assert_eq!(
            frame.data.pointer("/payload/entity"),
            Some(&Value::Null),
            "no synthetic entity matrix"
        );
    }
    Ok(())
}

fn find<'rows>(
    rows: &'rows [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> Result<(usize, &'rows MediaRecord), Box<dyn std::error::Error>> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == namespace && row.subresource.indices == indices
        })
        .ok_or_else(|| format!("missing {namespace} {indices:?}").into())
}

type Row = (u32, u16, u16, [f32; 3], [[f32; 4]; 3]);

fn nmc<const N: usize>(rows: [Row; N]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = vec![0; HEADER + N * STRIDE];
    put(&mut bytes, 4, &u32::try_from(N)?.to_le_bytes())?;
    for (index, (id, parent, kind, scale, transform)) in rows.into_iter().enumerate() {
        let start = HEADER + index * STRIDE;
        put(&mut bytes, start + 140, &id.to_le_bytes())?;
        put(&mut bytes, start + PARENT_AT, &parent.to_le_bytes())?;
        put(&mut bytes, start + 150, &kind.to_le_bytes())?;
        for (position, value) in transform.into_iter().flatten().enumerate() {
            put(
                &mut bytes,
                start + MATRIX_AT + 4 * position,
                &value.to_bits().to_le_bytes(),
            )?;
        }
        for (position, value) in scale.into_iter().enumerate() {
            put(
                &mut bytes,
                start + SCALE_AT + 4 * position,
                &value.to_bits().to_le_bytes(),
            )?;
        }
    }
    Ok(bytes)
}

fn put(bytes: &mut [u8], offset: usize, value: &[u8]) -> TestResult {
    bytes
        .get_mut(offset..offset + value.len())
        .ok_or("synthetic NMC field")?
        .copy_from_slice(value);
    Ok(())
}

const COLLISION_NODES: [Row; 4] = [
    (
        912,
        2,
        3,
        [1.0; 3],
        [
            [1.0, 0.0, 0.0, 4.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
    ),
    (600, 1, 0, [1.0; 3], [[0.0; 4]; 3]),
    (
        340,
        3,
        0,
        [2.0, 3.0, 4.0],
        [
            [0.0, -1.0, 0.0, 10.0],
            [1.0, 0.0, 0.0, 20.0],
            [0.0, 0.0, 1.0, 30.0],
        ],
    ),
    (
        6489,
        u16::MAX,
        0,
        [5.0, 2.0, 3.0],
        [
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0, -2.0],
            [0.0, 1.0, 0.0, 3.0],
        ],
    ),
];

pub(super) fn nmc_collision_fixture(
    truncated: bool,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let shape = collision_shape(4, &[0.0, 0.0, 0.0, 2.0]);
    let mut nodes = nmc(COLLISION_NODES)?;
    if !truncated {
        nodes.extend([0; FOOTER]);
    }
    let physical = physics_table(&[
        (0x0001_0000, &shape),
        (0x0002_0001, &shape),
        (0x0003_FFFF, &shape),
        (0x0001_0390, &shape),
    ])?;
    let file = TestChunkFile::ivo()
        .chunk(ivo::STAT_OBJ_PHYSICS.id(), 0x900, physical)
        .chunk(ivo::NODE_MESH_COMBO.id(), 0x900, nodes)
        .bytes();
    Ok(file)
}
