// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Source field faults survive retained rows and normal publication refuses them before creation.

use std::cell::Cell;
use std::error::Error;
use std::fs;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    CatalogError, CatalogFile, CatalogOptions, CatalogPublicationMode, Compression,
    ExportDiagnosticCode, Kind, build_catalog, write_catalog_from,
};
use p4kx_extraction::{Archive, DataForgeEntry, Filter};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestRecord, TestStruct, TestType,
    TestValue, entity_schema,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const MISSING: u128 = 0x9900_0000_0000_4000_8000_0000_0000_0001;

fn options() -> CatalogOptions<'static> {
    CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000))
}

fn archive(fault: bool) -> Result<(tempfile::TempDir, Archive), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database(fault)))
        .write_to(&path)?;
    Ok((root, Archive::open(&path)?))
}

fn database(fault: bool) -> Vec<u8> {
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Leaf"))
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(
            TestStruct::new("SCItemManufacturer")
                .field("Code", TestType::String)
                .field("fault", TestType::Bool)
                .field("optional", TestType::StrongPointer("Leaf".to_owned()))
                .field("empty", TestType::String)
                .field("link", TestType::Reference),
        )
        .record(
            TestRecord::new(
                1,
                "SCItemManufacturer",
                "SCItemManufacturer.Healthy",
                "libs/foundry/records/scitemmanufacturer/healthy.xml",
            )
            .value("Code", TestValue::text("OK")),
        )
        .record(inspected(fault))
        .record(TestRecord::new(
            3,
            "EntityClassDefinition",
            "EntityClassDefinition.DebugNoCrafter",
            "libs/foundry/records/entities/props/debug_no_crafter.xml",
        ));
    builder.build()
}

fn inspected(fault: bool) -> TestRecord {
    TestRecord::new(
        2,
        "SCItemManufacturer",
        "SCItemManufacturer.Inspected",
        "libs/foundry/records/scitemmanufacturer/inspected.xml",
    )
    .value("Code", TestValue::text("CHECK"))
    .value("empty", TestValue::text(""))
    .value(
        "fault",
        if fault {
            TestValue::Raw(vec![2])
        } else {
            TestValue::Bool(true)
        },
    )
    .value("link", TestValue::Reference(MISSING))
}

#[test]
fn retained_rows_keep_actual_field_failures_in_the_artifact() -> TestResult {
    let (_root, archive) = archive(true)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..options()
    };
    let built = build_catalog(&archive, &options)?;
    let summary = built.summary();
    assert!(
        summary.failures().is_empty(),
        "the typed mapper retained the rows without throwing a Record failure"
    );
    assert!(
        !summary.is_complete(),
        "an unreadable owned field remains a blocking read failure"
    );
    let health = summary.export_health();
    let kind = health
        .kind(Kind::Manufacturers)
        .ok_or("manufacturer health")?;
    assert_eq!(kind.written_rows, 2, "both rows remain serialized");
    assert_eq!(
        kind.source_records.projected, 2,
        "both primary typed projections completed"
    );
    assert_eq!(
        kind.source_records.omitted_after_read_failure, 0,
        "a retained field fault is not an omitted Record"
    );
    assert_eq!(
        health.failed_records.len(),
        1,
        "the exact physical source identity is retained without sampling"
    );
    assert!(
        health.diagnostics.iter().any(|diagnostic| diagnostic.code
            == ExportDiagnosticCode::UnreadableSource
            && diagnostic
                .examples
                .iter()
                .any(|example| example.field.as_deref() == Some("fault"))),
        "the failed field path is available from the artifact alone"
    );
    let document: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        document.get("exportHealth"),
        Some(&serde_json::to_value(health)?),
        "summary and file carry the same measured health"
    );
    Ok(())
}

#[test]
fn represented_optional_empty_and_dangling_states_do_not_invent_a_read_failure() -> TestResult {
    let (_root, archive) = archive(false)?;
    let built = build_catalog(&archive, &options())?;
    let health = built.summary().export_health();
    assert!(
        health.is_complete(),
        "represented source states remain healthy"
    );
    for code in [
        ExportDiagnosticCode::OptionalAbsent,
        ExportDiagnosticCode::EmptyValue,
        ExportDiagnosticCode::DanglingReference,
    ] {
        assert!(
            health
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code),
            "{code:?} remains a distinct represented state"
        );
    }
    assert!(
        health
            .unresolved_references
            .iter()
            .any(|reference| reference.target_guid.as_deref()
                == Some("99000000-0000-4000-8000-000000000001")),
        "the configured dangling identity is not sampled away"
    );
    Ok(())
}

#[test]
fn healthy_filtered_selection_and_enumerated_empty_kinds_publish_normally() -> TestResult {
    let (_root, archive) = archive(true)?;
    let filter = Filter::new("*healthy*");
    let options = CatalogOptions {
        filter: Some(&filter),
        ..options()
    };
    let built = build_catalog(&archive, &options)?;
    let health = built.summary().export_health();
    assert!(
        health.is_complete(),
        "an excluded faulty source does not corrupt a healthy selected kind"
    );
    let manufacturers = health
        .kind(Kind::Manufacturers)
        .ok_or("manufacturer health")?;
    assert_eq!(
        manufacturers.source_records.selected, 1,
        "the actual Filter selection is counted"
    );
    let empty = health.kind(Kind::Ships).ok_or("ship health")?;
    assert!(
        empty.source_inventory_complete,
        "the empty source selection was actually enumerated"
    );
    assert_eq!(
        empty.written_rows, 0,
        "empty kinds are not inferred failures"
    );
    assert!(
        !built
            .summary()
            .kind(Kind::Manufacturers)
            .ok_or("manufacturer summary")?
            .complete,
        "Filter completeness retains its existing meaning"
    );
    Ok(())
}

#[test]
fn normal_mode_refuses_memory_and_fresh_or_existing_files_before_creation() -> TestResult {
    let (root, archive) = archive(true)?;
    assert!(
        matches!(
            build_catalog(&archive, &options()),
            Err(CatalogError::IncompleteExport { .. })
        ),
        "normal in-memory publication also refuses incomplete bytes"
    );
    let entry = DataForgeEntry::read(&archive)?;
    for compression in [Compression::None, Compression::ZSTD] {
        for existing in [false, true] {
            let output = root
                .path()
                .join(format!("catalog-{compression:?}-{existing}.json"));
            if existing {
                fs::write(&output, b"previous complete output")?;
            }
            let called = Cell::new(false);
            let result = write_catalog_from(&archive, &entry, &options(), compression, || {
                called.set(true);
                CatalogFile::create(&output)
            });
            assert!(
                matches!(result, Err(CatalogError::IncompleteExport { .. })),
                "normal output refuses the same measured source fault"
            );
            assert!(!called.get(), "the destination callback is never invoked");
            if existing {
                assert_eq!(
                    fs::read(&output)?,
                    b"previous complete output",
                    "existing output remains intact"
                );
            } else {
                assert!(!output.exists(), "a refused fresh target is not created");
            }
        }
    }
    Ok(())
}

#[test]
fn a_leftover_entity_without_a_crafter_component_does_not_invent_a_successful_read() -> TestResult {
    let (_root, archive) = archive(false)?;
    let built = build_catalog(&archive, &options())?;
    let crafters = built
        .summary()
        .export_health()
        .kind(Kind::Crafters)
        .ok_or("crafter health")?;
    assert!(
        crafters.source_inventory_complete,
        "the candidate enumeration completed"
    );
    assert_eq!(
        crafters.source_records.selected, 0,
        "a negative component probe is not a selected crafter"
    );
    assert_eq!(
        crafters.source_records.read_successful, 0,
        "a negative probe is not a canonical Record read"
    );
    assert_eq!(
        crafters.source_records.projected, 0,
        "no typed crafter projection ran"
    );
    Ok(())
}

#[test]
fn explicit_diagnostic_memory_plain_and_zstd_outputs_share_the_same_health() -> TestResult {
    let (_root, archive) = archive(true)?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..options()
    };
    let memory = build_catalog(&archive, &options)?;
    for compression in [Compression::None, Compression::ZSTD] {
        let (summary, bytes) =
            write_catalog_from(&archive, &entry, &options, compression, || Ok(Vec::new()))?;
        let decoded = if compression == Compression::None {
            bytes
        } else {
            zstd::decode_all(bytes.as_slice())?
        };
        assert_eq!(
            decoded,
            memory.json(),
            "memory and streaming serialization agree exactly"
        );
        assert_eq!(
            summary.export_health(),
            memory.summary().export_health(),
            "compression does not alter measured health"
        );
        assert!(
            !summary.is_complete(),
            "successful diagnostic serialization is still incomplete"
        );
        assert_eq!(
            summary.export_health().publication_mode,
            CatalogPublicationMode::DiagnosticPartial,
            "the permission remains explicit in the artifact"
        );
    }
    Ok(())
}

#[test]
fn a_non_paint_physical_alias_does_not_become_a_selected_paint_input() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let database = entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "Paint", "libs/foundry/records/entities/scitem/paint.xml")
                .attach_type("Paints")
                .record(),
        )
        .record(
            TestEntity::new(1, "Tool", "libs/foundry/records/entities/scitem/tool.xml")
                .attach_type("Tool")
                .record(),
        )
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let built = build_catalog(&archive, &options())?;
    let health = built.summary().export_health();
    let items = health.kind(Kind::Items).ok_or("item health")?;
    let paints = health.kind(Kind::Paints).ok_or("paint health")?;
    assert_eq!(
        items.source_records.selected, 2,
        "both physical item Records are selected"
    );
    assert_eq!(
        paints.source_records.selected, 1,
        "only the actual paint projection supplies a paint input"
    );
    assert_eq!(
        paints.written_rows, 1,
        "the other physical alias does not invent a paint row"
    );
    Ok(())
}
