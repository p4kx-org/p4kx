// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, DataForgeEntry, FieldOrigin, InputScope, SourceInventory, SourceLocation,
};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};
use serde_json::Value;

use super::super::{ATTACHMENT_CONSTRAINTS, ITEM_BOUNDS, ITEM_PORTS};
use super::component_parity::parity;
use super::{Route, TestResult};

#[path = "../it/catalog_item_alignment_fixture.rs"]
#[expect(
    dead_code,
    reason = "The integration tests exercise the other fixture layouts"
)]
mod alignment;
#[path = "../it/equipment_bounds_fixture.rs"]
mod bounds;

const ALL: CatalogRecordSelection = CatalogRecordSelection::All;
const LANGUAGES: CatalogLanguageSelection = CatalogLanguageSelection::Selected(&[]);
const REMAINING: &str =
    "encrypted synthetic producer parity available; installed producer parity open";

pub(super) fn read() -> TestResult<[Route; 3]> {
    let [ports, attachments] = alignment_routes()?;
    Ok([ports, attachments, bounds_route()?])
}

fn alignment_routes() -> TestResult<[Route; 2]> {
    let (_directory, archive) =
        encrypted_archive(&alignment::database(alignment::Layout::Complete))?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog = catalog(&archive, &entry, CatalogPublicationMode::Normal)?;
    let session = CatalogQuerySession::from_entry(&archive, &entry, LANGUAGES, None)?;
    let parsed = session.parsed(None)?;
    let ports = parsed.with_item_ports(ALL, None, |snapshot| {
        let canonical = catalog
            .pointer(ITEM_PORTS)
            .ok_or("canonical port containers")?;
        assert_eq!(
            serde_json::to_value(snapshot.availability)?,
            canonical["availability"],
            "the same port declaration family is available"
        );
        assert_eq!(
            serde_json::to_value(&snapshot.rows)?,
            canonical["rows"],
            "all physical port and authored source cells match"
        );
        let first = snapshot.rows.first().ok_or("an original port container")?;
        let ports = first.ports().value().ok_or("the original port array")?;
        let port = ports
            .first()
            .and_then(|slot| slot.value())
            .ok_or("a physical port")?;
        check_origin(
            "port name",
            *first
                .owner_record_index()
                .value()
                .ok_or("physical owner index")?,
            port.name().origin(),
            &snapshot.source_inventory,
        );
        Route::queried_rows(ITEM_PORTS, &snapshot.rows, REMAINING)
    })??;
    let attachments = parsed.with_attachment_constraints(ALL, None, |snapshot| {
        parity(ATTACHMENT_CONSTRAINTS, &snapshot, &catalog)?;
        let row = snapshot
            .rows()
            .first()
            .ok_or("a physical attachment component")?;
        let definition = row
            .definition()
            .value()
            .ok_or("the original attachment definition")?;
        check_origin(
            "alignment axis",
            *row.owner_record_index()
                .value()
                .ok_or("physical owner index")?,
            definition.alignment_axis().origin(),
            snapshot.source_inventory(),
        );
        Route::queried_rows(ATTACHMENT_CONSTRAINTS, snapshot.rows(), REMAINING)
    })??;
    Ok([ports, attachments])
}

fn bounds_route() -> TestResult<Route> {
    let (_directory, archive) = encrypted_archive(&bounds::complete())?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog = catalog(&archive, &entry, CatalogPublicationMode::DiagnosticPartial)?;
    let session = CatalogQuerySession::from_entry(&archive, &entry, LANGUAGES, None)?;
    session.with_item_bounds(ALL, None, |snapshot| {
        parity(ITEM_BOUNDS, &snapshot, &catalog)?;
        let first = snapshot
            .rows()
            .first()
            .ok_or("a physical bounds component")?;
        check_origin(
            "bound component ordinal",
            *first
                .owner_record_index()
                .value()
                .ok_or("physical owner index")?,
            first.component_ordinal().origin(),
            snapshot.source_inventory(),
        );
        Route::queried_rows(ITEM_BOUNDS, snapshot.rows(), REMAINING)
    })?
}

fn catalog(
    archive: &Archive,
    entry: &DataForgeEntry,
    publication_mode: CatalogPublicationMode,
) -> TestResult<Value> {
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = publication_mode;
    Ok(serde_json::from_slice(
        build_catalog_from(archive, entry, &options)?.json(),
    )?)
}

fn encrypted_archive(database: &DataForgeBuilder) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("equipment_configurations.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    Ok((
        directory,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn check_origin(label: &str, owner: u32, origin: &FieldOrigin, inventory: &SourceInventory) {
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "{label} uses the encrypted DataForge"
    );
    assert!(
        inventory.iter_uses().any(|usage| {
            origin.source_uses_refs().contains(usage.id())
                && usage.scope() == InputScope::Payload
                && matches!(
                    usage.location(),
                    SourceLocation::Subresource { namespace, indices, field }
                        if namespace == "dataforgeValue"
                            && matches!(indices.as_slice(), [record, _] if *record == u64::from(owner))
                            && field == "$value"
                )
        }),
        "the authored use for {label} closes at the physical owner in the SDK result"
    );
}
