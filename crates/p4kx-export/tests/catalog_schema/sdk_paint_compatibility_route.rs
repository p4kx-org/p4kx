// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::paints::{
    PaintCompatibilitySnapshot, PaintCompatibilityStatus, PaintSelection,
};
use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::{Archive, DataForge, DataForgeEntry, Guid, InputScope, SourceUseRole, Step};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestPort, entity_schema,
};
use serde_json::Value;

use super::super::PAINTS;
use super::{Route, TestResult};

#[path = "../fixtures/owned_value_location.rs"]
mod owned_value_location;

use owned_value_location::value_location;

const PAINT: u128 = 0x1166_5401;
const SHIP: u128 = 0x1166_5402;

pub(super) fn read() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let snapshot = full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?
        .read_paint_compatibility(PaintSelection::RecordIndex(0), None)?;
    assert_eq!(
        snapshot.status,
        PaintCompatibilityStatus::Found,
        "the selected Paint and all inputs were read"
    );
    assert_eq!(snapshot.paint_record_indices, [0], "physical Paint owner");
    assert_eq!(snapshot.ship_record_indices, [1], "physical matching Ship");

    let row = snapshot
        .row
        .as_ref()
        .ok_or("the physically selected Paint")?;
    assert_eq!(
        row.item_guid(),
        "00000000-0000-0000-0000-000011665401".parse()?,
        "the row names the queried Paint"
    );
    let ship: Guid = "00000000-0000-0000-0000-000011665402".parse()?;
    assert!(
        row.ship_guids().eq([ship]),
        "the original Ship Record port contributes the sole fitting Ship"
    );
    let source = DataForgeEntry::read(&archive)?;
    verify_inputs(&snapshot, &source.open()?)?;

    let normal = build_catalog(
        &archive,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let document: Value = serde_json::from_slice(normal.json())?;
    let rows = document
        .get("paints")
        .and_then(Value::as_array)
        .ok_or("canonical Normal Paint array")?;
    assert_eq!(
        rows.as_slice(),
        &[serde_json::to_value(row)?],
        "the scoped row and the complete Normal export agree"
    );

    Route::queried_value(
        PAINTS,
        row,
        "installed matching Ship and cold memory witnessed, with full Normal parity, fleet source closure and Paint enumeration open",
    )
}

fn verify_inputs(snapshot: &PaintCompatibilitySnapshot, forge: &DataForge<'_>) -> TestResult {
    let compatibility = snapshot
        .compatibility_inputs
        .as_ref()
        .ok_or("the compatibility source scope")?;
    assert!(
        snapshot.paint_inputs.source_set_complete() && compatibility.source_set_complete(),
        "the Paint owner and matching inputs have complete source sets"
    );
    let ship_required_tags = value_location(forge, 1, |steps| {
        matches!(
            steps,
            [
                Step::Field("Components"),
                Step::Element(_),
                Step::Field("Ports"),
                Step::Element(_),
                Step::Field("RequiredPortTags"),
            ]
        )
    })?;
    let paint_tags = value_location(forge, 0, |steps| {
        matches!(
            steps,
            [
                Step::Field("Components"),
                Step::Element(_),
                Step::Field("AttachDef"),
                Step::Field("Tags"),
            ]
        )
    })?;
    let inventory = &snapshot.source_inventory;
    let ship_inputs = inventory.select_uses(compatibility.source_uses_refs())?;
    assert!(
        ship_inputs.iter_uses().any(|used| {
            used.location() == &ship_required_tags
                && used.scope() == InputScope::Payload
                && used.role() == SourceUseRole::Selector
        }),
        "the selected Ship port cites the exact original required tags"
    );
    let paint_inputs = inventory.select_uses(snapshot.paint_inputs.source_uses_refs())?;
    assert!(
        paint_inputs.iter_uses().any(|used| {
            used.location() == &paint_tags
                && used.scope() == InputScope::Payload
                && used.role() == SourceUseRole::Payload
        }),
        "the selected Paint cites the exact original Attachment tags"
    );
    Ok(())
}

fn database() -> DataForgeBuilder {
    const TAG: &str = "FixturePaintOffer";
    let paint = TestEntity::new(
        PAINT,
        "FixturePaint",
        "libs/foundry/records/entities/scitem/ships/paints/fixture_paint.xml",
    )
    .attach_type("Paints")
    .tags(TAG)
    .record();
    let ship = TestEntity::new(
        SHIP,
        "FixturePaintShip",
        "libs/foundry/records/entities/spaceships/fixture_paint_ship.xml",
    )
    .vehicle("", "")
    .port(
        TestPort::new("hardpoint_paint")
            .item_type::<&str>("Paints", [])
            .tags("", TAG),
    )
    .record();
    entity_schema(DataForgeBuilder::new().version(8))
        .record(paint)
        .record(ship)
}
