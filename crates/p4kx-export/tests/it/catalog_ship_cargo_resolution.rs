// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "catalog_ship_cargo_mixed_mounts.rs"]
mod mixed_mounts;

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogPublicationMode, ShipBaseOptions};
use p4kx_extraction::{Archive, DataForgeEntry, SourceInventory, SourceLocation, SourceUseRole};

use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[path = "catalog_ship_cargo_fixture.rs"]
mod fixture;

use fixture::{GRID_RECORD, database};

#[test]
fn ship_cargo_uses_the_first_physical_duplicate_for_each_mount() -> TestResult {
    for (first, second, expected) in [(125, 975, 2.5), (975, 125, 19.5)] {
        let database = database(first, second, false);
        let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
        let entry = DataForgeEntry::read(&archive)?;
        assert_inventory_order(&entry)?;
        assert_cargo(&archive, &entry, Some(expected), 0)?;
        let scoped_has_facility =
            equipment::with_cargo_grids_from(&archive, &entry, GRID_RECORD, None, |snapshot| {
                !snapshot.rows().is_empty()
            })?;
        assert!(
            !scoped_has_facility,
            "the attachable grid needs no independent facility component for ship aggregation"
        );
    }
    Ok(())
}

#[test]
fn ship_cargo_keeps_the_lower_bound_when_a_mounted_inventory_reference_is_dangling() -> TestResult {
    let database = database(125, 975, true);
    let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    assert_cargo(&archive, &entry, Some(2.5), 1)
}

#[test]
fn ship_cargo_keeps_the_first_invalid_duplicate_and_reports_unknown_capacity() -> TestResult {
    let database = database(-125, 975, false);
    let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    assert_cargo(&archive, &entry, None, 2)
}

fn assert_inventory_order(entry: &DataForgeEntry) -> TestResult {
    let forge = entry.open()?;
    let first = forge.record(0).ok_or("first original inventory Record")?;
    let second = forge.record(1).ok_or("second original inventory Record")?;
    assert_eq!(
        first.guid(),
        second.guid(),
        "the same authored inventory GUID"
    );
    assert_ne!(
        first.record_path()?,
        second.record_path()?,
        "two distinct physical definition paths"
    );
    assert_eq!(
        forge
            .records_by_guid(first.guid())
            .map(|record| record.index())
            .collect::<Vec<_>>(),
        [0, 1],
        "both physical aliases survive in stored order"
    );
    assert_eq!(
        forge
            .record_by_guid(first.guid())
            .ok_or("selected original inventory Record")?
            .index(),
        first.index(),
        "first physical definition is the existing aggregate's lookup"
    );
    Ok(())
}

fn assert_cargo(
    archive: &Archive,
    entry: &DataForgeEntry,
    expected_scu: Option<f64>,
    expected_issues: usize,
) -> TestResult {
    let ship_base =
        p4kx_export::build_ship_base_from(archive, entry, &ShipBaseOptions::new(UNIX_EPOCH))?;
    let [base] = ship_base.ships() else {
        return Err("one selected synthetic ship".into());
    };
    assert_eq!(
        base.cargo.scu, expected_scu,
        "direct Ship Base cargo uses every reached mount and the first physical definition"
    );
    assert_eq!(
        base.cargo.grids_with_issue, expected_issues,
        "one issue per reached mount without a usable capacity"
    );
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog = p4kx_export::build_catalog_from(archive, entry, options.with_full_provenance())?;
    let json: Value = serde_json::from_slice(catalog.json())?;
    let [row] = json
        .pointer("/projections/shipBaseFacts/rows")
        .and_then(Value::as_array)
        .ok_or("canonical Ship base facts list")?
        .as_slice()
    else {
        return Err("one original ship in the canonical projection".into());
    };
    assert_eq!(
        row.pointer("/cargo/scu/value")
            .ok_or("sourced cargo quantity")?,
        &json!(expected_scu),
        "canonical quantity agrees with Ship Base"
    );
    assert_eq!(
        row.pointer("/cargo/gridsWithIssue/value")
            .ok_or("sourced issue count")?,
        &json!(expected_issues),
        "canonical issue count agrees with Ship Base"
    );
    crate::health_origin_assertions::closure(&json)?;
    assert_sdk_cargo(archive, entry, row, expected_scu, expected_issues)
}

fn assert_sdk_cargo(
    archive: &Archive,
    entry: &DataForgeEntry,
    row: &Value,
    expected_scu: Option<f64>,
    expected_issues: usize,
) -> TestResult {
    full_proof::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_ship_sources(CatalogRecordSelection::All, None, |snapshot| {
        let [ship] = snapshot.rows() else {
            return Err("one SDK ship".into());
        };
        let cargo = ship.cargo();
        assert_eq!(
            row["cargo"],
            serde_json::to_value(cargo)?,
            "SDK and producer share the full sourced row"
        );
        assert_eq!(
            row["shipRecordIndex"],
            serde_json::to_value(ship.source().record_index())?,
            "the canonical row and SDK identify the same physical ship"
        );
        assert_eq!(cargo.scu().value().copied(), expected_scu, "SDK quantity");
        assert_eq!(
            cargo.grids_with_issue().value().copied(),
            Some(u64::try_from(expected_issues)?),
            "SDK issue count"
        );
        assert_eq!(
            cargo.scu().origin().contains_encrypted_source(),
            Some(true),
            "known contributors are in the encrypted original"
        );
        assert!(
            !cargo.scu().origin().source_set_complete(),
            "positive mounts do not prove all exclusion decisions"
        );
        let [left, right, remaining @ ..] = cargo.mounts() else {
            return Err("both mounts".into());
        };
        let half = expected_scu.map(|value| value / 2.0);
        for (mount, port, ordinal) in [(left, "grid_left", 0), (right, "grid_right", 1)] {
            assert_mount(
                mount,
                (port, ordinal),
                ship.source().record_index().value().copied(),
                half,
                snapshot.source_inventory(),
            )?;
        }
        assert_missing_grid(remaining);
        Ok::<(), Box<dyn std::error::Error>>(())
    })??;
    Ok(())
}

fn assert_mount(
    mount: &equipment::ShipCargoMountSource,
    (port, ordinal): (&str, u32),
    ship_record_index: Option<u32>,
    expected_capacity: Option<f64>,
    inventory: &SourceInventory,
) -> TestResult {
    assert_eq!(mount.port_path(), port, "the reached mount occurrence");
    let position = mount
        .position()
        .value()
        .ok_or("the resolved physical route")?;
    assert_eq!(
        position.port_ordinals(),
        &[ordinal],
        "the port ordinal retains the physical occurrence"
    );
    assert_eq!(
        Some(position.loadout_record_index()),
        ship_record_index,
        "the mount position references its root"
    );
    assert_eq!(
        mount.attachment().target_record_index().value(),
        Some(&GRID_RECORD),
        "the physical grid Entity is independent of the inventory Record"
    );
    assert_eq!(
        mount.storage().origin().contains_encrypted_source(),
        Some(true),
        "the chosen original storage declaration"
    );
    assert_eq!(
        mount.inventory_record_index().value(),
        Some(&0),
        "the first physical duplicate supplied capacity"
    );
    let capacity = mount.capacity_scu();
    assert_eq!(
        capacity.value().copied(),
        expected_capacity,
        "each physical mount contributes separately"
    );
    let conversions = inventory
        .iter_uses()
        .filter(|use_| capacity.origin().source_uses_refs().contains(use_.id()))
        .filter(|use_| use_.role() == SourceUseRole::Conversion)
        .collect::<Vec<_>>();
    let [amount] = conversions.as_slice() else {
        return Err("the selected original numeric amount".into());
    };
    assert!(
        matches!(amount.location(),
            SourceLocation::Subresource {namespace, indices, field}
            if namespace == "dataforgeValue" && matches!(indices.as_slice(), [0, _]) && field == "$value"
        ),
        "no later duplicate can contribute to the value"
    );
    Ok(())
}

fn assert_missing_grid(remaining: &[equipment::ShipCargoMountSource]) {
    if let [missing] = remaining {
        assert_eq!(
            missing.port_path(),
            "grid_unknown",
            "the unresolved grid occurrence"
        );
        assert!(
            missing.storage().value().is_some(),
            "authored dangling GUID is retained"
        );
        assert_eq!(
            missing.inventory_record_index().value(),
            None,
            "no invented physical Record"
        );
        assert!(
            !missing.capacity_scu().origin().source_set_complete(),
            "no invented operand provenance"
        );
    } else {
        assert!(remaining.is_empty(), "fixture reaches at most three grids");
    }
}
