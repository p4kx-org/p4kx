// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::chunks::ivo;
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestEntry, test_key};
use serde_json::{Value, json};

use super::model_physics_frame_provider::nmc_collision_fixture;
use super::model_vehicle_grid_provider::{FORGE, MODEL, schema, vehicle};
use super::support::{self, TestResult};

const COLLISION_TABLE: &str = "model-vehicle-grid-collision-table";
const COLLISION_SLOT: &str = "model-vehicle-grid-collision-slot";

#[test]
fn selected_proxy_links_canonical_slots_and_retains_the_original_affine_frame() -> TestResult {
    let root = tempfile::tempdir()?;
    let dcb = schema()
        .record(vehicle(1, "fixture.xml", 0, ["ship.cgf", "ship.cgf"]))
        .record(vehicle(2, "empty.xml", 0, ["empty.cgf", "newer.cgf"]))
        .build();
    let model = nmc_collision_fixture(false)?;
    let empty = TestChunkFile::ivo().bytes();
    let unsupported = TestChunkFile::ivo()
        .chunk(ivo::STAT_OBJ_PHYSICS.id(), 0x901, b"still retained")
        .bytes();
    let source = root.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(FORGE, dcb))
        .entry(TestEntry::zstd(MODEL, model.as_slice()).encrypted())
        .entry(TestEntry::stored("Data/empty.cgf", empty))
        .entry(TestEntry::stored("Data/newer.cgf", unsupported))
        .write_to(&source)?;
    let archive = Archive::open_with(source, OpenOptions::new().key(Some(test_key())))?;
    let output = root.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    assert_eq!(
        index.source_entries.written, 4,
        "shared proxy is retained once"
    );
    let base = output.join(&index.artifact_base);
    let rows = support::records(&output, &index, "model.jsonl")?;
    validate_schema(&rows)?;
    check_references(&rows)?;
    check_original_bytes(&rows, &base, &model)?;
    for (ordinal, row) in rows.iter().enumerate() {
        if row.subresource.namespace == COLLISION_SLOT && row.subresource.indices == [0, 0, 0, 0] {
            check_evidence(&base, ordinal)?;
        }
    }
    Ok(())
}

fn check_references(rows: &[MediaRecord]) -> TestResult {
    for component in 0..2 {
        let grid = find(rows, 0, "model-vehicle-grid", &[0, component])?;
        field(grid, "/payload/collision_state", &json!("tables-present"));
        field(
            grid,
            "/payload/path/resolution/target/source/entry/entry_index",
            &json!(1),
        );
        let table = find(rows, 0, COLLISION_TABLE, &[0, component, 0])?;
        field(
            table,
            "/payload/table",
            &json!({"namespace":"model-physics-table", "indices":[0]}),
        );
        for slot in 0..4 {
            let row = find(rows, 0, COLLISION_SLOT, &[0, component, 0, slot])?;
            field(
                row,
                "/payload/slot",
                &json!({"namespace":"model-physics-slot", "indices":[0,slot]}),
            );
            field(
                row,
                "/payload/model_frame",
                &json!({"namespace":"model-physics-frame", "indices":[0,slot]}),
            );
            field(row, "/payload/vehicle_placement", &json!("unavailable"));
            find(rows, 1, "model-physics-slot", &[0, slot])?;
            find(rows, 1, "model-physics-frame", &[0, slot])?;
        }
    }
    let grid = find(rows, 0, "model-vehicle-grid", &[1, 0])?;
    field(
        grid,
        "/payload/collision_state",
        &json!("no-physics-tables"),
    );
    assert!(
        find(rows, 0, COLLISION_TABLE, &[1, 0, 0]).is_err(),
        "an empty directory has no invented reference"
    );
    let grid = find(rows, 0, "model-vehicle-grid", &[1, 1])?;
    field(grid, "/payload/collision_state", &json!("tables-present"));
    find(rows, 0, COLLISION_TABLE, &[1, 1, 0])?;
    let table = find(rows, 3, "model-physics-table", &[0])?;
    field(table, "/payload/disposition", &json!("unsupported-version"));
    assert!(
        find(rows, 0, COLLISION_SLOT, &[1, 1, 0, 0]).is_err(),
        "no slots inferred from an unsupported table version"
    );
    Ok(())
}

fn check_original_bytes(rows: &[MediaRecord], base: &Path, model: &[u8]) -> TestResult {
    let frame = find(rows, 1, "model-physics-frame", &[0, 0])?;
    field(
        frame,
        "/payload/entity",
        &json!([
            [0.0, -3.0, 0.0, 10.0],
            [2.0, 0.0, 0.0, 28.0],
            [0.0, 0.0, 4.0, 30.0]
        ]),
    );
    let whole = find(rows, 1, "model-physics-frame", &[0, 2])?;
    field(whole, "/payload/disposition", &json!("whole-object"));
    field(whole, "/payload/entity", &Value::Null);
    let blob = frame
        .source
        .raw
        .as_ref()
        .ok_or("original proxy raw reference")?;
    assert_eq!(
        fs::read(base.join(&blob.path))?,
        model,
        "proxy artifact is byte-identical to the original"
    );
    Ok(())
}

fn check_evidence(generation: &Path, ordinal: usize) -> TestResult {
    let evidence = field_origin(generation, ordinal, "/data/payload/slot/indices/1")?;
    assert_eq!(
        evidence.get("containsEncryptedSource"),
        Some(&json!(true)),
        "encrypted target actually contributed"
    );
    assert_eq!(
        evidence.get("sourceSetComplete"),
        Some(&json!(true)),
        "both actual sources are known"
    );
    let ids = evidence
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("origin use references")?;
    let uses = read_rows(generation, "origin-uses.jsonl")?;
    let sources = read_rows(generation, "origin-sources.jsonl")?;
    for (field, owner) in [
        ("physicsSlotTableRow", 1),
        (
            "Components[VehicleComponentParams].physicsGrid.physicsProxy.path",
            0,
        ),
    ] {
        let found = ids
            .iter()
            .filter_map(|id| uses.iter().find(|evidence| evidence.get("id") == Some(id)))
            .filter(|evidence| {
                evidence.pointer("/location/field").and_then(Value::as_str) == Some(field)
            })
            .any(|evidence| {
                sources.iter().any(|input| {
                    input.get("id") == evidence.get("source")
                        && input.get("entryIndex") == Some(&json!(owner))
                })
            });
        assert!(
            found,
            "links cite original field {field} in physical Entry {owner}"
        );
    }
    let missing = field_origin(generation, ordinal, "/data/payload/vehicle_placement")?;
    assert_eq!(
        missing.get("sourceSetComplete"),
        Some(&json!(false)),
        "vehicle placement is unproved"
    );
    assert_eq!(
        missing.get("containsEncryptedSource"),
        Some(&Value::Null),
        "unknown placement has no fabricated encryption mark"
    );
    Ok(())
}

fn field_origin(base: &Path, ordinal: usize, field: &str) -> Result<Value, Box<dyn Error>> {
    let ordinal = json!(ordinal.to_string());
    read_rows(base, "field-origins.jsonl")?
        .into_iter()
        .find(|row| {
            row.get("field").and_then(Value::as_str) == Some(field)
                && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("model")
                && row.pointer("/scope/ordinal") == Some(&ordinal)
        })
        .ok_or_else(|| format!("no evidence for {field}").into())
}

fn read_rows(base: &Path, file: &str) -> Result<Vec<Value>, Box<dyn Error>> {
    fs::read_to_string(base.join(file))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn find<'a>(
    rows: &'a [MediaRecord],
    owner: usize,
    namespace: &str,
    indices: &[u64],
) -> Result<&'a MediaRecord, Box<dyn Error>> {
    rows.iter().find(|row| matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.entry_index == owner)
        && row.subresource.namespace == namespace && row.subresource.indices == indices)
        .ok_or_else(|| format!("unavailable source {owner}, namespace {namespace}, indices {indices:?}").into())
}

fn field(row: &MediaRecord, field: &str, expected: &Value) {
    assert_eq!(
        row.data.pointer(field),
        Some(expected),
        "exact source field {field}"
    );
}

fn validate_schema(rows: &[MediaRecord]) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|cause| cause.to_string())?;
    }
    Ok(())
}

fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Model],
        ..ExportOptions::default()
    }
}
