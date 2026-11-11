// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Recovered root directories cannot establish complete catalog source inventories.

use std::cell::Cell;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogError, CatalogFile, CatalogOptions, CatalogPublicationMode, Compression,
    ExportDiagnosticCode, ExportStageStatus, Kind, build_catalog, write_catalog_from,
};
use p4kx_extraction::{Archive, DataForgeEntry, EntryIssue, OpenOptions};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const CENTRAL: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
const ZIP64_END: [u8; 4] = [0x50, 0x4b, 0x06, 0x06];

fn archive(damaged: bool) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("SCItemManufacturer").field("Code", TestType::String))
        .record(
            TestRecord::new(
                1,
                "SCItemManufacturer",
                "SCItemManufacturer.One",
                "libs/foundry/records/scitemmanufacturer/one.xml",
            )
            .value("Code", TestValue::text("ONE")),
        )
        .build();
    let mut bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database))
        .entry(TestEntry::stored(
            "Data\\unneeded.xml",
            b"<unused/>".as_slice(),
        ))
        .build();
    if damaged {
        remove_directory_record_and_end(&mut bytes)?;
    }
    std::fs::write(&path, bytes)?;
    let archive = Archive::open_with(&path, OpenOptions::new().recover(true))?;
    assert_eq!(
        archive.entries().len(),
        if damaged { 1 } else { 2 },
        "the recovered fixture genuinely loses an Entry while retaining readable DCB data"
    );
    assert_eq!(
        archive
            .issues()
            .iter()
            .any(|(_, issue)| matches!(issue, EntryIssue::Recovered)),
        damaged,
        "only actual recovery sets the root-wide marker"
    );
    Ok((directory, archive))
}

fn remove_directory_record_and_end(bytes: &mut Vec<u8>) -> TestResult {
    let central: Vec<_> = bytes
        .windows(CENTRAL.len())
        .enumerate()
        .filter_map(|(index, value)| (value == CENTRAL).then_some(index))
        .collect();
    assert_eq!(
        central.len(),
        2,
        "the fixture has exactly two directory records"
    );
    let missing = *central.get(1).ok_or("second central record")?;
    bytes
        .get_mut(missing..missing + CENTRAL.len())
        .ok_or("central signature")?
        .fill(0);
    let end = bytes
        .windows(ZIP64_END.len())
        .rposition(|value| value == ZIP64_END)
        .ok_or("ZIP64 end")?;
    bytes.truncate(end);
    Ok(())
}

#[test]
fn an_actual_recovered_directory_refuses_normal_in_memory_publication() -> TestResult {
    let (_directory, archive) = archive(true)?;
    let result = build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH));
    assert!(
        matches!(result, Err(CatalogError::IncompleteExport(_))),
        "a recovered subset cannot establish successful absence for any catalog kind"
    );
    Ok(())
}

#[test]
fn recovered_diagnostic_output_has_one_global_marker_and_only_observed_counts() -> TestResult {
    let (_directory, archive) = archive(true)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog(&archive, &options)?;
    let health = built.summary().export_health();
    assert!(
        !health.is_complete(),
        "recovery uncertainty survives copying the catalog"
    );
    assert_eq!(
        health.unique_selected_source_records, 1,
        "the one readable manufacturer source is observed once"
    );
    assert!(
        health.failed_records().is_empty() && health.failed_entries().is_empty(),
        "archive-wide uncertainty is not assigned to a fabricated Record or Entry zero"
    );
    let global: Vec<_> = health
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.kind.is_none()
                && diagnostic.code == ExportDiagnosticCode::IncompleteInventory
        })
        .collect();
    assert_eq!(
        global.len(),
        1,
        "the root marker is not cloned into every kind"
    );
    let marker = global.first().ok_or("global marker")?;
    assert_eq!(
        marker.warning_code.as_deref(),
        Some("recovered_archive_directory"),
        "a stable technical code identifies actual root recovery"
    );
    assert_eq!(marker.count, 1, "unknown lost Entry counts are not guessed");
    assert!(
        marker.examples().is_empty(),
        "the archive-wide diagnostic fabricates no physical source identity"
    );
    assert_unknown_inventories(health)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        catalog.get("exportHealth"),
        Some(&serde_json::to_value(health)?),
        "the file-only roundtrip retains the exact uncertainty"
    );
    Ok(())
}

fn assert_unknown_inventories(health: &p4kx_export::ExportHealth) -> TestResult {
    for kind in health.kinds() {
        assert!(
            !kind.source_inventory_complete,
            "the root directory may hide inputs of any kind"
        );
        assert_eq!(
            (kind.read, kind.projection),
            (ExportStageStatus::Incomplete, ExportStageStatus::Incomplete),
            "observed lower bounds do not establish complete required reads"
        );
        assert_eq!(
            kind.source_records.failed_selection_candidates, 0,
            "root uncertainty does not invent a failed selection Record"
        );
    }
    let manufacturers = health
        .kind(Kind::Manufacturers)
        .ok_or("manufacturer health")?;
    assert_eq!(
        (
            manufacturers.source_records.selected,
            manufacturers.written_rows
        ),
        (1, 1),
        "readable data remains retained with its actual units"
    );
    Ok(())
}

#[test]
fn recovered_normal_writes_preserve_fresh_and_existing_destinations() -> TestResult {
    let (directory, archive) = archive(true)?;
    let entry = DataForgeEntry::read(&archive)?;
    for compression in [Compression::None, Compression::ZSTD] {
        for existing in [false, true] {
            let path = directory
                .path()
                .join(format!("catalog-{compression:?}-{existing}.json"));
            if existing {
                std::fs::write(&path, b"previous healthy snapshot")?;
            }
            let created = Cell::new(false);
            let result = write_catalog_from(
                &archive,
                &entry,
                &CatalogOptions::new(UNIX_EPOCH),
                compression,
                || {
                    created.set(true);
                    CatalogFile::create(&path)
                },
            );
            assert!(
                matches!(result, Err(CatalogError::IncompleteExport(_))),
                "root recovery blocks both plain and compressed normal writes"
            );
            assert!(!created.get(), "the destination factory is not called");
            if existing {
                assert_eq!(
                    std::fs::read(&path)?,
                    b"previous healthy snapshot",
                    "the existing output is unchanged"
                );
            } else {
                assert!(!path.exists(), "a fresh destination is not created");
            }
        }
    }
    Ok(())
}

#[test]
fn allowing_recovery_with_a_healthy_directory_keeps_normal_publication_healthy() -> TestResult {
    let (_directory, archive) = archive(false)?;
    let built = build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
    let health = built.summary().export_health();
    assert!(
        health.is_complete(),
        "the option alone is not evidence of actual recovery"
    );
    assert!(
        health
            .kinds()
            .iter()
            .all(|kind| kind.source_inventory_complete),
        "a regular intact directory has no root inventory uncertainty"
    );
    Ok(())
}
