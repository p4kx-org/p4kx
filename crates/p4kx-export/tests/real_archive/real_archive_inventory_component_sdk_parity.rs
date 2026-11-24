// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::equipment::EquipmentVisitSummary;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, Build};
use serde::Serialize;

use super::component_family_parity::{
    ComponentRows, expected_leftovers, physical_slot, require_complete,
};
use super::equipment_input;
use crate::real_archive_catalog_table_parity::{
    PhysicalSlot, StreamedComponentDigests, TestResult, with_bound_measurement_from_producer,
};
use crate::real_archive_verification::print_line;

#[derive(Clone, Copy)]
enum Family {
    ItemPorts,
    ItemBounds,
    AttachmentConstraints,
    InventoryConfigurations,
}

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::ItemPorts => "/configurations/itemPorts",
            Self::ItemBounds => "/configurations/itemBounds",
            Self::AttachmentConstraints => "/configurations/itemAttachmentConstraints",
            Self::InventoryConfigurations => "/configurations/inventoryConfigurations",
        }
    }
}

const FAMILIES: [Family; 4] = [
    Family::ItemPorts,
    Family::ItemBounds,
    Family::AttachmentConstraints,
    Family::InventoryConfigurations,
];
const TABLES: usize = FAMILIES.len();
const PRODUCER: &str = "P4KX_INVENTORY_COMPONENT_CATALOG_PRODUCER_REVISION";
const COLD_DATASET: &str = "P4KX_INVENTORY_COMPONENT_SDK_COLD_DATASET";
const LEFTOVERS: &str = "P4KX_INVENTORY_COMPONENT_SDK_EXPECTED_LEFTOVERS";

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_inventory_component_tables_match_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    let expected_leftovers = expected_leftovers(LEFTOVERS)?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let scratch = tempfile::tempdir()?;
        let mut evidence = StreamedComponentDigests::for_tables(scratch.path());
        let availability = with_parsed(archive, |parsed, build| {
            let mut availability = [DatasetAvailability::Unavailable; TABLES];
            for (table, (family, state)) in FAMILIES.into_iter().zip(&mut availability).enumerate()
            {
                let capture = Capture {
                    table,
                    evidence: &mut evidence,
                };
                *state =
                    InventoryWitness::new(family.path(), build, expected_leftovers, Some(capture))
                        .query(parsed, family)?;
            }
            Ok(availability)
        })?;
        let counts = measurement.verify_streamed_component_tables(
            FAMILIES.map(Family::path),
            availability,
            evidence,
        )?;
        print_line(&format!(
            "complete Inventory and attachment components SDK parity: rows={:?} sourceUses={} physicalSources={}",
            counts.rows, counts.source_uses, counts.physical_sources,
        ));
        Ok(())
    })
}

#[test]
#[ignore = "installed Archive: configure the expected Build and one exact dataset path"]
fn installed_inventory_component_cold_query() -> TestResult {
    let path = env::var(COLD_DATASET)?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path() == path)
        .ok_or("expected one exact Inventory and attachment component dataset path")?;
    let expected_leftovers = expected_leftovers(LEFTOVERS)?;
    equipment_input::with_archive(|archive| {
        with_parsed(archive, |parsed, build| {
            InventoryWitness::new(family.path(), build, expected_leftovers, None)
                .query(parsed, family)
                .map(|_| ())
        })
    })
}

fn with_parsed<T>(
    archive: &Archive,
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Build) -> TestResult<T>,
) -> TestResult<T> {
    let build = archive
        .build()
        .ok_or("the installed Archive lacks a Build")?;
    let session =
        CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
    inspect(&session.parsed(None)?, build)
}

struct Capture<'scope, 'directory> {
    table: usize,
    evidence: &'scope mut StreamedComponentDigests<'directory, TABLES>,
}

struct InventoryWitness<'scope, 'directory> {
    path: &'static str,
    build: &'scope Build,
    expected_leftovers: usize,
    capture: Option<Capture<'scope, 'directory>>,
    last_owner: Option<u32>,
    last_slot: Option<PhysicalSlot>,
    owners: usize,
    rows: usize,
    uses: usize,
    sources: usize,
}

impl<'scope, 'directory> InventoryWitness<'scope, 'directory> {
    const fn new(
        path: &'static str,
        build: &'scope Build,
        expected_leftovers: usize,
        capture: Option<Capture<'scope, 'directory>>,
    ) -> Self {
        Self {
            path,
            build,
            expected_leftovers,
            capture,
            last_owner: None,
            last_slot: None,
            owners: 0,
            rows: 0,
            uses: 0,
            sources: 0,
        }
    }

    fn query(
        mut self,
        parsed: &ParsedCatalogQueries<'_>,
        family: Family,
    ) -> TestResult<DatasetAvailability> {
        let selection = CatalogRecordSelection::All;
        let summary = match family {
            Family::ItemPorts => parsed.visit_item_ports(selection, None, |index, snapshot| {
                let rows = ComponentRows {
                    build: snapshot.build.as_ref(),
                    availability: snapshot.availability,
                    rows: &snapshot.rows,
                    coverage: &snapshot.coverage,
                    inventory: &snapshot.source_inventory,
                };
                self.inspect(Some(index), &rows, |row| {
                    physical_slot(row.owner_record_index(), row.component_ordinal())
                })
            })?,
            Family::ItemBounds => {
                parsed.visit_item_bounds(selection, None, |index, snapshot| {
                    self.inspect(Some(index), &ComponentRows::equipment(snapshot), |row| {
                        physical_slot(row.owner_record_index(), row.component_ordinal())
                    })
                })?
            }
            Family::AttachmentConstraints => {
                parsed.visit_attachment_constraints(selection, None, |index, snapshot| {
                    self.inspect(Some(index), &ComponentRows::equipment(snapshot), |row| {
                        physical_slot(row.owner_record_index(), row.component_ordinal())
                    })
                })?
            }
            Family::InventoryConfigurations => {
                return parsed.with_inventory_configurations(selection, None, |snapshot| {
                    self.inspect(None, &ComponentRows::equipment(&snapshot), |row| {
                        let owner = row.owner();
                        physical_slot(owner.record_index(), owner.component_ordinal())
                    })?;
                    self.finish(snapshot.availability(), snapshot.coverage())
                })?;
            }
        };
        self.finish_visits(&summary)
    }

    fn inspect<Row: Serialize>(
        &mut self,
        index: Option<u32>,
        snapshot: &ComponentRows<'_, Row>,
        slot: impl Fn(&Row) -> TestResult<PhysicalSlot>,
    ) -> TestResult {
        let &ComponentRows {
            build,
            availability,
            rows,
            coverage,
            inventory,
        } = snapshot;
        if build != Some(self.build) {
            return Err(format!("{}: component query Build does not match", self.path).into());
        }
        require_complete(self.path, availability, rows.len(), coverage)?;
        if let Some(index) = index {
            if self.last_owner.is_some_and(|previous| previous >= index) {
                return Err(format!("{}: unordered or repeated physical owner", self.path).into());
            }
            require_leftovers(self.path, coverage, 0)?;
            self.last_owner = Some(index);
            increment(&mut self.owners, 1)?;
        }
        self.inspect_slots(index, rows, &slot)?;
        let uses = inventory.use_count();
        let sources = inventory.source_count();
        if !rows.is_empty() && (uses == 0 || sources == 0) {
            return Err(format!(
                "{}: physical rows lack original source inventory",
                self.path
            )
            .into());
        }
        if let Some(capture) = &mut self.capture {
            capture
                .evidence
                .retain_table_owner(capture.table, rows, slot, inventory)?;
        }
        increment(&mut self.rows, rows.len())?;
        increment(&mut self.uses, uses)?;
        increment(&mut self.sources, sources)
    }

    fn inspect_slots<Row>(
        &mut self,
        index: Option<u32>,
        rows: &[Row],
        slot: impl Fn(&Row) -> TestResult<PhysicalSlot>,
    ) -> TestResult {
        for row in rows {
            let current = slot(row)?;
            if index.is_some_and(|owner| owner != current.record_index)
                || self.last_slot.is_some_and(|previous| previous >= current)
            {
                return Err(format!(
                    "{}: foreign, duplicate or unordered physical slot at {current:?}",
                    self.path
                )
                .into());
            }
            self.last_slot = Some(current);
        }
        Ok(())
    }

    fn finish_visits(&self, summary: &EquipmentVisitSummary) -> TestResult<DatasetAvailability> {
        if summary.owner_count() != self.owners || summary.row_count() != self.rows {
            return Err(format!(
                "{}: terminal counts differ from physical callbacks",
                self.path
            )
            .into());
        }
        self.finish(summary.availability(), summary.coverage())
    }

    fn finish(
        &self,
        availability: DatasetAvailability,
        coverage: &CatalogRecordCoverage,
    ) -> TestResult<DatasetAvailability> {
        require_complete(self.path, availability, self.rows, coverage)?;
        require_leftovers(self.path, coverage, self.expected_leftovers)?;
        let path = self.path;
        print_line(&format!(
            "Inventory and attachment components SDK: dataset={path} availability={availability:?} rows={} inventoryUses={} inventorySources={} skippedLeftovers={}",
            self.rows, self.uses, self.sources, coverage.skipped_leftovers,
        ));
        if path != Family::InventoryConfigurations.path() {
            print_line(&format!(
                "inventory component visitor: dataset={path} owners={} inventoryCountBasis=owner-occurrences",
                self.owners,
            ));
        }
        Ok(availability)
    }
}

fn increment(total: &mut usize, count: usize) -> TestResult {
    *total = total
        .checked_add(count)
        .ok_or("inventory witness counter overflow")?;
    Ok(())
}

fn require_leftovers(path: &str, coverage: &CatalogRecordCoverage, expected: usize) -> TestResult {
    if coverage.skipped_leftovers != expected {
        return Err(format!(
            "{path}: {} excluded Entities differ from {expected} reviewed canonical exclusions",
            coverage.skipped_leftovers
        )
        .into());
    }
    Ok(())
}

#[test]
fn inventory_visitor_requires_exact_canonical_exclusions() {
    let path = Family::ItemBounds.path();
    let mut coverage = CatalogRecordCoverage::default();
    coverage.skipped_leftovers = 4;
    assert!(
        require_leftovers(path, &coverage, 4).is_ok(),
        "reviewed canonical exclusions"
    );
    assert!(
        require_leftovers(path, &coverage, 0).is_err(),
        "unexpected exclusions"
    );
    assert!(
        require_leftovers(path, &coverage, 5).is_err(),
        "missing canonical exclusions"
    );
}
