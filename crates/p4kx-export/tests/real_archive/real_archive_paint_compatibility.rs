// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::paints::{
    PaintCompatibilitySnapshot, PaintCompatibilityStatus, PaintSelection,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};
use p4kx_extraction::{
    DataForge, DataForgeEntry, DataForgeOrigin, FieldOrigin, Guid, InputScope, PhysicalSourceKind,
    SourceInventory,
};
use p4kx_extraction::{SourceLocation, SourceUseRole, Step};

use super::{equipment_input, report_scoped_rows};
use crate::real_archive_catalog_table_parity::{TestResult, wire_row};
use crate::real_archive_paints::{CNOU_MUSTANG_OMEGA, PAINT_MUSTANG_OMEGA};

#[path = "../fixtures/owned_value_location.rs"]
mod owned_value_location;

#[test]
#[ignore = "real Archive: configure the exact Build containing the known Paint and Ship"]
fn installed_paint_compatibility_keeps_original_record_evidence() -> TestResult {
    equipment_input::with_archive(|archive| {
        let entry = equipment_input::dataforge(archive)?;
        let original = OriginalPaintWitness::read(&entry)?;
        let session = CatalogQuerySession::from_entry(
            archive,
            &entry,
            CatalogLanguageSelection::Selected(&[]),
            None,
        )
        .map_err(|_| "open the original Paint query session")?;
        let snapshot = session
            .read_paint_compatibility(PaintSelection::RecordIndex(original.paint_index), None)
            .map_err(|_| "read the selected original Paint compatibility")?;
        if snapshot.build.as_ref() != archive.build()
            || snapshot.paint_record_indices != [original.paint_index]
            || snapshot.status != PaintCompatibilityStatus::Found
        {
            return Err(
                "the nominated physical Paint needs complete compatibility on its Build".into(),
            );
        }
        check_inputs(&snapshot, &original)?;
        let row = snapshot
            .row
            .as_ref()
            .ok_or("the nominated Paint row is missing")?;
        if row.item_guid() != original.paint_guid
            || snapshot.ship_record_indices != [original.ship_index]
            || !row.ship_guids().eq([original.ship_guid])
        {
            return Err("the exact Paint must fit only the nominated original Ship".into());
        }
        report_scoped_rows("Paint compatibility", "installed-source", &[wire_row(row)?])
    })
}

struct OriginalPaintWitness {
    archive_entry: usize,
    paint_index: u32,
    paint_guid: Guid,
    paint_tags: SourceLocation,
    ship_index: u32,
    ship_guid: Guid,
    ship_vehicle: SourceLocation,
}

impl OriginalPaintWitness {
    fn read(entry: &DataForgeEntry) -> TestResult<Self> {
        let DataForgeOrigin::Archive(original) = entry.origin() else {
            return Err("the selected DataForge does not come from the original Archive".into());
        };
        let archive_entry = usize::try_from(original.binding().entry_index())?;
        let forge = entry
            .open()
            .map_err(|_| "decode the original DataForge records")?;
        let paint_guid = PAINT_MUSTANG_OMEGA.parse()?;
        let ship_guid = CNOU_MUSTANG_OMEGA.parse()?;
        let paint_index = unique_record_index(&forge, paint_guid)?;
        let ship_index = unique_record_index(&forge, ship_guid)?;
        let paint_tags = owned_value_location::value_location(&forge, paint_index, |steps| {
            matches!(
                steps,
                [
                    Step::Field("Components"),
                    Step::Element(_),
                    Step::Field("AttachDef"),
                    Step::Field("Tags"),
                ]
            )
        })
        .map_err(|_| "the nominated Paint must own one exact Attachment Tags coordinate")?;
        let ship_vehicle = owned_value_location::value_location(&forge, ship_index, |steps| {
            matches!(
                steps,
                [
                    Step::Field("Components"),
                    Step::Element(_),
                    Step::Field("vehicleDefinition"),
                ]
            )
        })
        .map_err(|_| "the nominated Ship must own one exact vehicle definition coordinate")?;
        Ok(Self {
            archive_entry,
            paint_index,
            paint_guid,
            paint_tags,
            ship_index,
            ship_guid,
            ship_vehicle,
        })
    }
}

fn unique_record_index(forge: &DataForge<'_>, guid: Guid) -> TestResult<u32> {
    let mut records = forge.records_by_guid(guid);
    let record = records
        .next()
        .ok_or("the nominated Record is missing from the configured Build")?;
    if records.next().is_some() {
        return Err("the nominated GUID has more than one physical Record".into());
    }
    Ok(record.index())
}

fn check_inputs(
    snapshot: &PaintCompatibilitySnapshot,
    original: &OriginalPaintWitness,
) -> TestResult {
    let compatibility = snapshot
        .compatibility_inputs
        .as_ref()
        .ok_or("the selected Paint has no compatibility evidence")?;
    if !snapshot.paint_inputs.source_set_complete() || !compatibility.source_set_complete() {
        return Err("the selected Paint has incomplete original input coverage".into());
    }
    check_coordinate(
        &snapshot.source_inventory,
        &snapshot.paint_inputs,
        original.archive_entry,
        &original.paint_tags,
        SourceUseRole::Payload,
    )?;
    check_coordinate(
        &snapshot.source_inventory,
        compatibility,
        original.archive_entry,
        &original.ship_vehicle,
        SourceUseRole::Selector,
    )
}

fn check_coordinate(
    inventory: &SourceInventory,
    origin: &FieldOrigin,
    archive_entry: usize,
    coordinate: &SourceLocation,
    role: SourceUseRole,
) -> TestResult {
    let selected = inventory.select_uses(origin.source_uses_refs())?;
    let mut matching = selected.iter_uses().filter(|used| {
        used.location() == coordinate && used.scope() == InputScope::Payload && used.role() == role
    });
    let used = matching
        .next()
        .ok_or("the scope omits the exact original coordinate")?;
    if matching.next().is_some() {
        return Err("the exact original coordinate is ambiguous".into());
    }
    let source = selected
        .iter_sources()
        .find(|source| source.id() == used.source())
        .ok_or("the exact coordinate has no original physical source")?;
    if !matches!(source.kind(), PhysicalSourceKind::ArchiveEntry(entry) if entry.entry_index() == archive_entry)
    {
        return Err("the exact coordinate names a different Archive Entry".into());
    }
    Ok(())
}
