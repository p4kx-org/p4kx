// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Source field faults survive retained rows and normal publication refuses them before creation.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::error::Error;
use std::fs;

use p4kx_export::{
    CatalogError, CatalogFile, CatalogOptions, CatalogPublicationMode, Compression,
    ExportDiagnosticCode, ExportDiagnosticStage, ExportHealth, ExportSourceRole, Kind,
    build_catalog, write_catalog_from,
};
use p4kx_extraction::{Archive, DataForgeEntry, Filter};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, entity_schema};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

#[path = "export_health_fixture.rs"]
mod fixture;

#[test]
fn retained_rows_keep_actual_field_failures_in_the_artifact() -> TestResult {
    let (_root, archive) = fixture::archive(true)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..fixture::options()
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
    assert_failed_identity(health);
    let document: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        document.get("exportHealth"),
        Some(&serde_json::to_value(health)?),
        "summary and file carry the same measured health"
    );
    Ok(())
}

fn assert_failed_identity(health: &ExportHealth) {
    let identities: BTreeSet<_> = health
        .failed_records
        .iter()
        .map(|failed| {
            (
                failed.record_index,
                failed.guid.as_str(),
                failed.record_path.as_deref(),
                failed.kind.map(Kind::name),
                failed.role,
            )
        })
        .collect();
    assert_eq!(
        identities.len(),
        1,
        "exactly one physical identity failed; stage entries are not extra Records"
    );
    let expected = (
        1,
        "00000000-0000-0000-0000-000000000002",
        Some("libs/foundry/records/scitemmanufacturer/inspected"),
        Some(Kind::Manufacturers.name()),
        ExportSourceRole::Selected,
    );
    assert!(
        identities.contains(&expected),
        "the one actual failed Record retains exact path/kind/role"
    );
    for stage in [
        ExportDiagnosticStage::SourceField,
        ExportDiagnosticStage::Projection,
    ] {
        assert!(
            health
                .failed_records
                .iter()
                .any(|failed| failed.stage == stage
                    && failed.code == ExportDiagnosticCode::UnreadableSource),
            "both actual source and typed projection failure stages are retained"
        );
    }
    assert!(
        health.diagnostics.iter().any(|diagnostic| diagnostic.code
            == ExportDiagnosticCode::UnreadableSource
            && diagnostic
                .examples
                .iter()
                .any(|example| example.field.as_deref() == Some("Code"))),
        "the failed field path is available from the artifact alone"
    );
}

#[test]
fn represented_optional_empty_and_dangling_states_do_not_invent_a_read_failure() -> TestResult {
    let (_root, archive) = fixture::archive(false)?;
    let built = build_catalog(&archive, &fixture::options())?;
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
    let (_root, archive) = fixture::archive(true)?;
    let filter = Filter::new("*healthy*");
    let options = CatalogOptions {
        filter: Some(&filter),
        ..fixture::options()
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
    let (root, archive) = fixture::archive(true)?;
    assert!(
        matches!(
            build_catalog(&archive, &fixture::options()),
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
            let result =
                write_catalog_from(&archive, &entry, &fixture::options(), compression, || {
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
    let (_root, archive) = fixture::archive(false)?;
    let built = build_catalog(&archive, &fixture::options())?;
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
    let (_root, archive) = fixture::archive(true)?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..fixture::options()
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
    let built = build_catalog(&archive, &fixture::options())?;
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
