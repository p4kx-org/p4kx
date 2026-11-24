// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::env;

use p4kx_export::catalog_queries::equipment::{ComponentOwner, EquipmentSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, SourceInventory};
use serde::Serialize;

use super::equipment_input;
use crate::real_archive_catalog_table_parity::{
    DigestedInventory, DigestedTable, PhysicalSlot, TestResult,
    with_bound_measurement_from_producer,
};
use crate::real_archive_verification::print_line;

#[derive(Clone, Copy)]
enum Family {
    Durability,
    Degradation,
    Distortion,
    Emp,
}

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::Durability => "/configurations/durability",
            Self::Degradation => "/configurations/degradation",
            Self::Distortion => "/configurations/distortion",
            Self::Emp => "/configurations/emp",
        }
    }
}

const FAMILIES: [Family; 4] = [
    Family::Durability,
    Family::Degradation,
    Family::Distortion,
    Family::Emp,
];

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_item_condition_tables_match_one_complete_catalog() -> TestResult {
    let producer = env::var("P4KX_ITEM_CONDITION_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Item condition Catalog producer revision")?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let retained = retained_inputs(archive)?;
        let counts = measurement.verify_digested_tables_with_availability(
            FAMILIES.map(Family::path),
            &retained.tables,
            retained.availability,
            &retained.inventory,
        )?;
        print_line(&format!(
            "complete Item condition SDK parity: rows={:?} sourceUses={} physicalSources={}",
            counts.rows, counts.source_uses, counts.physical_sources,
        ));
        Ok(())
    })
}

struct DigestCapture<'scope> {
    table: &'scope mut DigestedTable,
    inventory: &'scope mut DigestedInventory,
}

struct RetainedInputs {
    tables: [DigestedTable; FAMILIES.len()],
    availability: [DatasetAvailability; FAMILIES.len()],
    inventory: DigestedInventory,
}

fn retained_inputs(archive: &Archive) -> TestResult<RetainedInputs> {
    let session =
        CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let mut tables = FAMILIES.map(|_| DigestedTable::default());
    let mut availability = [DatasetAvailability::Unavailable; FAMILIES.len()];
    let mut inventory = DigestedInventory::default();
    for ((family, table), expected) in FAMILIES.into_iter().zip(&mut tables).zip(&mut availability)
    {
        *expected = read_family(
            archive,
            &parsed,
            family,
            Some(DigestCapture {
                table,
                inventory: &mut inventory,
            }),
        )?;
    }
    Ok(RetainedInputs {
        tables,
        availability,
        inventory,
    })
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact dataset path"]
fn installed_item_condition_cold_query() -> TestResult {
    let path = env::var("P4KX_ITEM_CONDITION_SDK_COLD_DATASET")
        .map_err(|_| "configure one canonical Item condition dataset path")?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path() == path)
        .ok_or("expected one exact Item condition dataset path")?;
    equipment_input::with_archive(|archive| {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        let _ = read_family(archive, &parsed, family, None)?;
        Ok(())
    })
}

fn read_family(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    family: Family,
    capture: Option<DigestCapture<'_>>,
) -> TestResult<DatasetAvailability> {
    let witness = QueryWitness {
        archive,
        family,
        capture,
    };
    let selection = CatalogRecordSelection::All;
    match family {
        Family::Durability => parsed.with_durability(selection, None, |snapshot| {
            witness.inspect(&snapshot, |row| row.owner())
        })?,
        Family::Degradation => parsed.with_degradation(selection, None, |snapshot| {
            witness.inspect(&snapshot, |row| row.owner())
        })?,
        Family::Distortion => parsed.with_distortion(selection, None, |snapshot| {
            witness.inspect(&snapshot, |row| row.owner())
        })?,
        Family::Emp => parsed.with_emp(selection, None, |snapshot| {
            witness.inspect(&snapshot, |row| row.owner())
        })?,
    }
}

struct QueryWitness<'scope> {
    archive: &'scope Archive,
    family: Family,
    capture: Option<DigestCapture<'scope>>,
}

impl QueryWitness<'_> {
    fn inspect<Row: Serialize>(
        self,
        snapshot: &EquipmentSnapshot<'_, Row>,
        owner: impl Fn(&Row) -> &ComponentOwner,
    ) -> TestResult<DatasetAvailability> {
        let path = self.family.path();
        if snapshot.build().is_none() || snapshot.build() != self.archive.build() {
            return Err(format!("{path}: Item condition Build does not match").into());
        }
        let rows = snapshot.rows();
        let availability = snapshot.availability();
        let coverage = snapshot.coverage();
        require_complete(path, availability, rows.len(), coverage)?;
        let inventory = snapshot.source_inventory();
        let uses = inventory.use_count();
        let sources = inventory.source_count();
        if !rows.is_empty() && (uses == 0 || sources == 0) {
            return Err(
                format!("{path}: physical rows lack their original source inventory").into(),
            );
        }

        match self.capture {
            Some(capture) => capture.retain(rows, owner, inventory)?,
            None => check_cold_physical_slots(rows, owner)?,
        }
        print_line(&format!(
            "Item condition SDK: dataset={path} availability={availability:?} rows={} inventoryUses={uses} inventorySources={sources} skippedLeftovers={}",
            rows.len(),
            coverage.skipped_leftovers,
        ));
        Ok(availability)
    }
}

impl DigestCapture<'_> {
    fn retain<Row: Serialize>(
        self,
        rows: &[Row],
        owner: impl Fn(&Row) -> &ComponentOwner,
        inventory: &SourceInventory,
    ) -> TestResult {
        if rows.is_empty() {
            return Ok(());
        }
        let mut references = HashSet::new();
        for row in rows {
            self.table
                .retain_direct(physical_slot(owner(row))?, row, &mut references)?;
        }
        self.inventory.retain(inventory, references)
    }
}

fn check_cold_physical_slots<Row>(
    rows: &[Row],
    owner: impl Fn(&Row) -> &ComponentOwner,
) -> TestResult {
    let mut previous = None;
    for row in rows {
        let slot = physical_slot(owner(row))?;
        if previous.is_some_and(|prior| prior >= slot) {
            return Err(
                "the cold Item condition query has unordered or repeated physical slots".into(),
            );
        }
        previous = Some(slot);
    }
    Ok(())
}

fn physical_slot(owner: &ComponentOwner) -> TestResult<PhysicalSlot> {
    Ok(PhysicalSlot {
        record_index: *owner
            .record_index()
            .value()
            .ok_or("missing original Item Record index")?,
        component_ordinal: *owner
            .component_ordinal()
            .value()
            .ok_or("missing original Item Components ordinal")?,
    })
}

fn require_complete(
    path: &str,
    availability: DatasetAvailability,
    rows: usize,
    coverage: &CatalogRecordCoverage,
) -> TestResult {
    match availability {
        DatasetAvailability::Available => {}
        DatasetAvailability::Absent if rows == 0 => {}
        _ => {
            return Err(
                format!("{path}: incomplete Item condition declaration availability").into(),
            );
        }
    }
    if !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
    {
        return Err(format!(
            "{path}: incomplete Item condition coverage (unreadableHeaders={} unreadableBodies={} incompleteFields={})",
            coverage.unreadable_headers.len(),
            coverage.unreadable_bodies.len(),
            coverage.incomplete_fields.len(),
        )
        .into());
    }
    Ok(())
}

#[test]
fn item_condition_parity_distinguishes_empty_absent_and_incomplete() {
    let path = Family::Durability.path();
    let mut coverage = CatalogRecordCoverage::default();
    for (availability, rows, complete) in [
        (DatasetAvailability::Available, 1, true),
        (DatasetAvailability::Available, 0, true),
        (DatasetAvailability::Absent, 0, true),
        (DatasetAvailability::Absent, 1, false),
        (DatasetAvailability::Unavailable, 0, false),
    ] {
        assert_eq!(
            require_complete(path, availability, rows, &coverage).is_ok(),
            complete,
            "{availability:?} with {rows} rows must preserve its declaration state",
        );
    }
    coverage.skipped_leftovers = 1;
    assert!(
        require_complete(path, DatasetAvailability::Available, 1, &coverage).is_ok(),
        "a classified catalog exclusion leaves the selected coverage complete",
    );
    coverage.unreadable_headers.push(0);
    assert!(
        require_complete(path, DatasetAvailability::Absent, 0, &coverage).is_err(),
        "an unreadable header cannot establish absence",
    );
    coverage.unreadable_headers.clear();
    coverage.unreadable_bodies.push(0);
    assert!(
        require_complete(path, DatasetAvailability::Available, 1, &coverage).is_err(),
        "an unreadable body cannot establish complete coverage",
    );
    coverage.unreadable_bodies.clear();
    coverage.incomplete_fields.push(0);
    assert!(
        require_complete(path, DatasetAvailability::Available, 1, &coverage).is_err(),
        "an incompletely projected field cannot establish complete coverage",
    );
}
