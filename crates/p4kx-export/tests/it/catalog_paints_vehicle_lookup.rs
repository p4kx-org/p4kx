// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::paints::{
    PaintCompatibilitySnapshot, PaintCompatibilityStatus, PaintSelection, read_paint_compatibility,
};
use p4kx_extraction::{
    Archive, DataForgeEntry, InputScope, OpenOptions, SourceLocation, SourceUseRole, Step,
};

use super::{TestResult, guid};
use crate::paints_fixture::vehicle_lookup::{VehicleLookupInput, write_vehicle_lookup_archive};
use crate::paints_fixture::{PAINT_ALPHA_RED, SHIP_MISSING};

#[path = "../fixtures/owned_value_location.rs"]
mod owned_value_location;

use owned_value_location::value_location;

#[test]
fn scoped_paint_distinguishes_verified_missing_unreadable_and_recovered_vehicle_inputs()
-> TestResult {
    for (input, expected, read_failed) in [
        (
            VehicleLookupInput::Missing,
            PaintCompatibilityStatus::Found,
            false,
        ),
        (
            VehicleLookupInput::Malformed,
            PaintCompatibilityStatus::Partial,
            true,
        ),
        (
            VehicleLookupInput::Encrypted,
            PaintCompatibilityStatus::Partial,
            true,
        ),
        (
            VehicleLookupInput::Recovered,
            PaintCompatibilityStatus::Partial,
            false,
        ),
    ] {
        let directory = tempfile::tempdir()?;
        let archive = Archive::open_with(
            write_vehicle_lookup_archive(directory.path(), input)?,
            OpenOptions::new().recover(true),
        )?;
        let snapshot = read_paint_compatibility(&archive, PaintSelection::RecordIndex(0), None)?;
        assert_eq!(snapshot.status, expected, "{input:?}");
        assert_eq!(snapshot.paint_record_indices, [0], "{input:?}");
        assert_eq!(snapshot.ship_record_indices, [1], "{input:?}");
        let dependencies = snapshot
            .vehicle_dependencies
            .as_ref()
            .ok_or("vehicle lookup coverage")?;
        assert_eq!(
            !dependencies.failed_entries().is_empty(),
            read_failed,
            "{input:?}"
        );
        assert_eq!(
            dependencies.is_complete(),
            input == VehicleLookupInput::Missing,
            "{input:?}"
        );
        assert_eq!(
            serde_json::to_value(snapshot.row.as_ref().ok_or("the original Paint row")?)?,
            serde_json::json!({"itemGuid": guid(PAINT_ALPHA_RED), "shipGuids": [guid(SHIP_MISSING)]}),
            "an independent original Record port still supplies the observed fit"
        );
        if input == VehicleLookupInput::Missing {
            check_missing_origin(&archive, &snapshot)?;
        }
    }
    Ok(())
}

fn check_missing_origin(archive: &Archive, snapshot: &PaintCompatibilitySnapshot) -> TestResult {
    let origin = snapshot
        .compatibility_inputs
        .as_ref()
        .ok_or("compatibility inputs")?;
    assert!(
        origin.source_set_complete(),
        "a trusted directory establishes that the selected XML is absent"
    );
    let selected = snapshot
        .source_inventory
        .select_uses(origin.source_uses_refs())?;
    let source = DataForgeEntry::read(archive)?;
    let caller = value_location(&source.open()?, 1, |steps| {
        matches!(
            steps,
            [
                Step::Field("Components"),
                Step::Element(_),
                Step::Field("vehicleDefinition"),
            ]
        )
    })?;
    assert!(
        selected.iter_uses().any(|used| matches!(
            used.location(),
            SourceLocation::ArchiveDirectoryPaths { .. }
        )),
        "missing XML cites the original root directory paths"
    );
    assert!(
        selected.iter_uses().any(|used| {
            used.location() == &caller
                && used.scope() == InputScope::Payload
                && used.role() == SourceUseRole::Selector
        }),
        "missing XML cites the exact original vehicle selector"
    );
    Ok(())
}
