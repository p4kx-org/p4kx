// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::env;

use p4kx_export::catalog_queries::equipment::{ComponentOwner, EquipmentSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, DatasetAvailability,
    ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, Build, SourceInventory, SourcedValue};
use serde::Serialize;

use super::equipment_input;
use crate::real_archive_catalog_table_parity::{
    DigestedInventory, DigestedTable, PhysicalSlot, TestResult,
    with_bound_measurement_from_producer,
};
use crate::real_archive_verification::print_line;

pub(super) trait PhysicalComponentFamily: Copy {
    const DOMAIN: &'static str;

    fn path(self) -> &'static str;

    fn query(
        self,
        parsed: &ParsedCatalogQueries<'_>,
        witness: QueryWitness<'_>,
    ) -> TestResult<DatasetAvailability>;
}

pub(super) fn verify_retained<F: PhysicalComponentFamily, const N: usize>(
    families: [F; N],
    producer_variable: &str,
) -> TestResult {
    let producer = env::var(producer_variable)?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let retained = retained_inputs(archive, families)?;
        let counts = measurement.verify_digested_tables_with_availability(
            families.map(F::path),
            &retained.tables,
            retained.availability,
            &retained.inventory,
        )?;
        print_line(&format!(
            "complete {} SDK parity: rows={:?} sourceUses={} physicalSources={}",
            F::DOMAIN,
            counts.rows,
            counts.source_uses,
            counts.physical_sources,
        ));
        Ok(())
    })
}

pub(super) fn verify_cold<F: PhysicalComponentFamily, const N: usize>(
    families: [F; N],
    dataset_variable: &str,
) -> TestResult {
    let path = env::var(dataset_variable)?;
    let family = families
        .into_iter()
        .find(|family| family.path() == path)
        .ok_or_else(|| format!("expected one exact {} dataset path", F::DOMAIN))?;
    equipment_input::with_archive(|archive| {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        let witness = QueryWitness {
            archive,
            domain: F::DOMAIN,
            path: family.path(),
            capture: None,
        };
        family.query(&parsed, witness)?;
        Ok(())
    })
}

struct DigestCapture<'scope> {
    table: &'scope mut DigestedTable,
    inventory: &'scope mut DigestedInventory,
}

struct RetainedInputs<const N: usize> {
    tables: [DigestedTable; N],
    availability: [DatasetAvailability; N],
    inventory: DigestedInventory,
}

fn retained_inputs<F: PhysicalComponentFamily, const N: usize>(
    archive: &Archive,
    families: [F; N],
) -> TestResult<RetainedInputs<N>> {
    let session =
        CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let mut tables = families.map(|_| DigestedTable::default());
    let mut availability = [DatasetAvailability::Unavailable; N];
    let mut inventory = DigestedInventory::default();
    for ((family, table), expected) in families.into_iter().zip(&mut tables).zip(&mut availability)
    {
        let witness = QueryWitness {
            archive,
            domain: F::DOMAIN,
            path: family.path(),
            capture: Some(DigestCapture {
                table,
                inventory: &mut inventory,
            }),
        };
        *expected = family.query(&parsed, witness)?;
    }
    Ok(RetainedInputs {
        tables,
        availability,
        inventory,
    })
}

pub(super) struct QueryWitness<'scope> {
    archive: &'scope Archive,
    domain: &'static str,
    path: &'static str,
    capture: Option<DigestCapture<'scope>>,
}

pub(super) struct ComponentRows<'scope, Row> {
    pub(super) build: Option<&'scope Build>,
    pub(super) availability: DatasetAvailability,
    pub(super) rows: &'scope [Row],
    pub(super) coverage: &'scope CatalogRecordCoverage,
    pub(super) inventory: &'scope SourceInventory,
}

impl<'scope, Row> ComponentRows<'scope, Row> {
    pub(super) fn equipment(snapshot: &'scope EquipmentSnapshot<'_, Row>) -> Self {
        Self {
            build: snapshot.build(),
            availability: snapshot.availability(),
            rows: snapshot.rows(),
            coverage: snapshot.coverage(),
            inventory: snapshot.source_inventory(),
        }
    }
}

impl QueryWitness<'_> {
    pub(super) fn inspect<Row: Serialize>(
        self,
        snapshot: &EquipmentSnapshot<'_, Row>,
        owner: impl Fn(&Row) -> &ComponentOwner,
    ) -> TestResult<DatasetAvailability> {
        self.inspect_rows(&ComponentRows::equipment(snapshot), |row| {
            let owner = owner(row);
            physical_slot(owner.record_index(), owner.component_ordinal())
        })
    }

    pub(super) fn inspect_rows<Row: Serialize>(
        self,
        snapshot: &ComponentRows<'_, Row>,
        slot: impl Fn(&Row) -> TestResult<PhysicalSlot>,
    ) -> TestResult<DatasetAvailability> {
        let path = self.path;
        let &ComponentRows {
            build,
            availability,
            rows,
            coverage,
            inventory,
        } = snapshot;
        if build.is_none() || build != self.archive.build() {
            return Err(format!("{path}: component query Build does not match").into());
        }
        require_complete(path, availability, rows.len(), coverage)?;
        let uses = inventory.use_count();
        let sources = inventory.source_count();
        if !rows.is_empty() && (uses == 0 || sources == 0) {
            return Err(
                format!("{path}: physical rows lack their original source inventory").into(),
            );
        }

        match self.capture {
            Some(capture) => capture.retain(rows, slot, inventory)?,
            None => check_cold_physical_slots(rows, slot)?,
        }
        print_line(&format!(
            "{} SDK: dataset={path} availability={availability:?} rows={} inventoryUses={uses} inventorySources={sources} skippedLeftovers={}",
            self.domain,
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
        slot: impl Fn(&Row) -> TestResult<PhysicalSlot>,
        inventory: &SourceInventory,
    ) -> TestResult {
        if rows.is_empty() {
            return Ok(());
        }
        let mut references = HashSet::new();
        for row in rows {
            self.table.retain_direct(slot(row)?, row, &mut references)?;
        }
        self.inventory.retain(inventory, references)
    }
}

fn check_cold_physical_slots<Row>(
    rows: &[Row],
    slot: impl Fn(&Row) -> TestResult<PhysicalSlot>,
) -> TestResult {
    let mut previous = None;
    for row in rows {
        let current = slot(row)?;
        if previous.is_some_and(|prior| prior >= current) {
            return Err(format!(
                "the cold component query has an unordered or repeated physical slot at {current:?}"
            )
            .into());
        }
        previous = Some(current);
    }
    Ok(())
}

pub(super) fn physical_slot(
    record_index: &SourcedValue<u32>,
    component_ordinal: &SourcedValue<u32>,
) -> TestResult<PhysicalSlot> {
    Ok(PhysicalSlot {
        record_index: *record_index
            .value()
            .ok_or("missing original owner Record index")?,
        component_ordinal: *component_ordinal
            .value()
            .ok_or("missing original Components ordinal")?,
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
                format!("{path}: incomplete component query declaration availability").into(),
            );
        }
    }
    if !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
    {
        return Err(format!(
            "{path}: incomplete component query coverage (unreadableHeaders={} unreadableBodies={} incompleteFields={})",
            coverage.unreadable_headers.len(),
            coverage.unreadable_bodies.len(),
            coverage.incomplete_fields.len(),
        )
        .into());
    }
    Ok(())
}

#[test]
fn component_coverage_distinguishes_empty_absent_and_incomplete() {
    let path = "/configurations/durability";
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
