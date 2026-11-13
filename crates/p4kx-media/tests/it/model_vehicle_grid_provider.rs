// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::query::MAX_PATH_VALUES;
use p4kx_extraction::{Archive, Filter, OpenOptions};
use p4kx_media::{ExportOptions, MediaError, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

use super::model_source_provider::field_uses;
use super::support::{self, TestResult};

const STREAM: &str = "model.jsonl";
const MODEL: &str = "Data/ship.cgf";
const FORGE: &str = "Data/Game2.dcb";

#[test]
fn vehicle_grids_keep_physical_record_occurrences_and_target_resolution() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let source = temporary.path().join("Data.p4k");
    let dcb = schema()
        .record(vehicle(1, "Synthetic/Z.xml", 1, ["ship.cgf", "double.cgf"]))
        .record(vehicle(2, "Synthetic/A.xml", 1, ["missing.cgf", ""]))
        .build();
    let unread = |name| TestEntry::stored(name, b"excluded target bytes").with_crc32c(1);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(FORGE, dcb.as_slice()).encrypted())
        .entry(unread(MODEL))
        .entry(unread("Data/double.cgf"))
        .entry(unread("data/DOUBLE.CGF"))
        .write_to(&source)?;
    let archive = Archive::open_with(source, OpenOptions::new().key(Some(test_key())))?;
    let output = temporary.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options(FORGE)?)?)?;
    assert_eq!(
        index.source_entries.written, 1,
        "grid links never promote filtered targets"
    );
    let rows = support::records(&output, &index, STREAM)?;
    let base = output.join(&index.artifact_base);
    check_retained(&rows, &base, &dcb)?;
    check_references(&rows, &base)?;
    Ok(())
}

fn check_retained(rows: &[MediaRecord], base: &Path, dcb: &[u8]) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    assert_eq!(
        rows.len(),
        5,
        "one database and four original path declarations"
    );
    for row in rows {
        assert!(
            schema.is_valid(&serde_json::to_value(row)?),
            "closed vehicle grid row schema"
        );
        let source = row.source.raw.as_ref().ok_or("retained database")?;
        assert_eq!(
            fs::read(base.join(&source.path))?,
            dcb,
            "all rows retain their DCB owner bytes"
        );
    }
    Ok(())
}

fn check_references(rows: &[MediaRecord], base: &Path) -> TestResult {
    for (indices, path, status, stored_filename) in [
        ([0, 1], "ship.cgf", "not-selected", "Synthetic/Z.xml"),
        ([0, 2], "double.cgf", "ambiguous", "Synthetic/Z.xml"),
        ([1, 1], "missing.cgf", "missing", "Synthetic/A.xml"),
        ([1, 2], "", "unavailable", "Synthetic/A.xml"),
    ] {
        let (ordinal, row) = rows
            .iter()
            .enumerate()
            .find(|(_, row)| {
                row.subresource.namespace == "model-vehicle-grid"
                    && row.subresource.indices == indices
            })
            .ok_or("source grid occurrence")?;
        for (field, value) in [
            ("/payload/path/original/value", json!(path)),
            ("/payload/path/resolution/status", json!(status)),
            ("/payload/record_file", json!(stored_filename)),
        ] {
            assert_eq!(row.data.pointer(field), Some(&value), "exact field {field}");
        }
        let evidence = field_uses(base, ordinal, "/data/payload/path/original/value", 0, true)?;
        for namespace in ["dataforgeDecodedBytes", "dataforgeArrayElement"] {
            assert!(
                evidence.iter().any(|site| {
                    site.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                }),
                "path provenance includes original {namespace}"
            );
        }
    }
    Ok(())
}

#[test]
fn an_unselected_dataforge_is_not_read_for_a_model_only_filter() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let file = p4kx_testkit::TestChunkFile::ivo().bytes();
    let archive = support::archive_with(
        temporary.path(),
        [
            TestEntry::stored(FORGE, b"unavailable").with_crc32c(1),
            TestEntry::stored(MODEL, file),
        ],
    )?;
    let output = temporary.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options(MODEL)?)?)?;
    let rows = support::records(&output, &index, STREAM)?;
    assert_eq!(
        index.source_entries.written, 1,
        "a filtered database has no payload read"
    );
    assert!(
        rows.iter()
            .all(|row| !row.subresource.namespace.starts_with("model-vehicle-grid")),
        "no synthetic grid absence claims"
    );
    Ok(())
}

#[test]
fn complete_grid_export_uses_caller_work_budget_above_the_interactive_query_rule() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let original = schema()
        .record(vehicle(
            1,
            "Synthetic/Many.xml",
            MAX_PATH_VALUES,
            ["missing.cgf"],
        ))
        .build();
    let archive = support::archive_with(temporary.path(), [TestEntry::stored(FORGE, original)])?;
    let output = temporary.path().join("out");
    let mut settings = options(FORGE)?;
    let index = support::published(p4kx_media::export(&archive, &output, &settings)?)?;
    let rows = support::records(&output, &index, STREAM)?;
    assert!(
        rows.iter()
            .any(|row| row.subresource.namespace == "model-vehicle-grid"
                && row.subresource.indices == [0, MAX_PATH_VALUES as u64]),
        "a matching component beyond the interactive row limit remains on export"
    );
    settings.limits.metadata_work = MAX_PATH_VALUES as u64;
    let limited_output = temporary.path().join("limited");
    let refused = p4kx_media::export(&archive, &limited_output, &settings);
    let Err(MediaError::Source { cause, .. }) = refused else {
        return Err("an unreadable admitted source must return the typed error".into());
    };
    assert!(
        matches!(
            *cause,
            MediaError::Limit {
                budget: "metadata_work",
                ..
            }
        ),
        "a caller-defined budget refusal retains the exact typed cause"
    );
    Ok(())
}

fn options(filter: &str) -> Result<ExportOptions, Box<dyn std::error::Error>> {
    Ok(ExportOptions {
        kinds: vec![MediaKind::Model],
        include: Some(Filter::parse(filter)?),
        ..ExportOptions::default()
    })
}

fn schema() -> DataForgeBuilder {
    let class = |name: &str| TestType::Class(name.to_owned());
    DataForgeBuilder::new()
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ))
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(TestStruct::new("UnrelatedComponent").parent("DataForgeComponentParams"))
        .structure(
            TestStruct::new("VehicleComponentParams")
                .parent("DataForgeComponentParams")
                .field("physicsGrid", class("PhysicsGrid")),
        )
        .structure(TestStruct::new("PhysicsGrid").field("physicsProxy", class("Geometry")))
        .structure(TestStruct::new("Geometry").field("path", TestType::String))
}

fn vehicle<const N: usize>(
    guid: u128,
    file: &str,
    preceding: usize,
    paths: [&str; N],
) -> TestRecord {
    let mut components =
        vec![TestValue::Struct(TestInstance::new("UnrelatedComponent")); preceding];
    components.extend(paths.into_iter().map(|path| {
        TestValue::Struct(TestInstance::new("VehicleComponentParams").value(
            "physicsGrid",
            TestValue::Struct(TestInstance::new("PhysicsGrid").value(
                "physicsProxy",
                TestValue::Struct(
                    TestInstance::new("Geometry").value("path", TestValue::text(path)),
                ),
            )),
        ))
    }));
    TestRecord::new(guid, "EntityClassDefinition", "Fixture", file)
        .value("Components", TestValue::Array(components))
}
