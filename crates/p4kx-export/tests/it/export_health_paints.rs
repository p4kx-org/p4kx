// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Paint dependencies retain actual vehicle evidence without unrelated loadout operations.

use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogError, CatalogOptions, CatalogPublicationMode, EntryOperation, ExportStageStatus, Kind,
    build_catalog,
};
use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout, entity_schema,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const VEHICLE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/paint_carrier.xml";

fn archive(
    vehicle_fault: bool,
    loadout_fault: bool,
) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let database = entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(
                1,
                "PaintCarrier",
                "libs/foundry/records/entities/spaceships/paint_carrier.xml",
            )
            .attach_type("NOITEM_Vehicle")
            .vehicle(VEHICLE, "")
            .loadout(TestLoadout::Xml("synthetic/loadout.xml".to_owned()))
            .record(),
        )
        .record(
            TestEntity::new(2, "Paint", "libs/foundry/records/entities/scitem/paint.xml")
                .attach_type("Paints")
                .record(),
        )
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database))
        .entry(TestEntry::stored(
            format!("Data/{VEHICLE}"),
            if vehicle_fault {
                b"<broken>".as_slice()
            } else {
                b"<Vehicle/>".as_slice()
            },
        ))
        .entry(TestEntry::stored(
            "Data/synthetic/loadout.xml",
            if loadout_fault {
                b"<broken>".as_slice()
            } else {
                b"<Loadout/>".as_slice()
            },
        ))
        .write_to(&path)?;
    Ok((directory, Archive::open(&path)?))
}

#[test]
fn unrelated_loadout_failure_does_not_invalidate_healthy_paint_vehicle_facts() -> TestResult {
    let (_directory, archive) = archive(false, true)?;
    let filter = Filter::new("*paint*");
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog(&archive, &options)?;
    let health = built.summary().export_health();
    assert!(
        health
            .failed_entries()
            .iter()
            .any(|failure| failure.kind() == Kind::Ships
                && failure.operation() == EntryOperation::XmlLoadout),
        "the unrelated XML failure actually occurred"
    );
    let ship = health.kind(Kind::Ships).ok_or("ship health")?;
    assert_eq!(
        (
            ship.written_rows,
            ship.source_records.omitted_after_read_failure,
            ship.source_records.omitted_after_projection_failure
        ),
        (1, 0, 0),
        "a retained ship with a failed XML dependency stays a written ship"
    );
    let paints = health.kind(Kind::Paints).ok_or("paint health")?;
    assert!(
        paints.source_inventory_complete,
        "healthy paint vehicle configuration is fully enumerated"
    );
    assert_eq!(
        (paints.read, paints.projection),
        (ExportStageStatus::Complete, ExportStageStatus::Complete),
        "an incidental loadout operation does not become a paint requirement"
    );
    assert_eq!(
        paints.source_entries.dependencies, 1,
        "only the actual vehicle file is required by paints"
    );
    assert_eq!(
        paints.source_entries.failed_dependencies, 0,
        "the vehicle byte read and parse both completed"
    );
    Ok(())
}

#[test]
fn required_vehicle_parse_failure_still_blocks_paints_and_normal_publication() -> TestResult {
    let (_directory, archive) = archive(true, false)?;
    let normal = CatalogOptions::new(UNIX_EPOCH);
    assert!(
        matches!(
            build_catalog(&archive, &normal),
            Err(CatalogError::IncompleteExport(_))
        ),
        "normal publication refuses the actual vehicle failure"
    );
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..normal
    };
    let built = build_catalog(&archive, &options)?;
    let paints = built
        .summary()
        .export_health()
        .kind(Kind::Paints)
        .ok_or("paint health")?;
    assert!(
        !paints.source_inventory_complete,
        "unreadable vehicle configuration can hide paint ports"
    );
    assert_eq!(
        paints.source_entries.failed_dependencies, 1,
        "the physical vehicle input is failed once"
    );
    assert!(
        paints
            .failed_entries()
            .iter()
            .any(|failure| failure.operation() == EntryOperation::VehicleImplementation),
        "the required parse operation and exact Entry identity are retained"
    );
    Ok(())
}

#[test]
fn a_paints_only_filter_does_not_read_excluded_vehicle_or_loadout_files() -> TestResult {
    let (_directory, archive) = archive(true, true)?;
    let filter = Filter::new("*/scitem/*");
    let options = CatalogOptions {
        filter: Some(&filter),
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog(&archive, &options)?;
    let health = built.summary().export_health();
    assert!(
        health.is_complete(),
        "excluded broken dependencies do not make the selected paint scope unhealthy"
    );
    let paints = health.kind(Kind::Paints).ok_or("paint health")?;
    assert_eq!(
        paints.written_rows, 1,
        "the selected paint remains exported"
    );
    assert_eq!(
        paints.source_entries.dependencies, 0,
        "excluded vehicle facts cause no actual file reads"
    );
    Ok(())
}
