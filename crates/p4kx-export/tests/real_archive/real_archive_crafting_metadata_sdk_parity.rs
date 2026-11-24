// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::env;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    CatalogRecordSource, DatasetAvailability, ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, Build, Guid, SourceInventory, SourcedValue};
use serde::Serialize;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{self, Origins, Table, TestResult};
use crate::real_archive_scoped_equipment_sdk::{component_family_parity, equipment_input};
use crate::real_archive_verification::print_line;

#[derive(Clone, Copy)]
enum Family {
    Global,
    Categories,
    CategoryLists,
}

const FAMILIES: [Family; 3] = [Family::Global, Family::Categories, Family::CategoryLists];

struct Snapshot<'scope, Row> {
    build: Option<&'scope Build>,
    availability: DatasetAvailability,
    rows: &'scope [Row],
    coverage: &'scope CatalogRecordCoverage,
    inventory: &'scope SourceInventory,
}

impl Family {
    const fn path(self) -> &'static str {
        match self {
            Self::Global => "/configurations/craftingGlobal",
            Self::Categories => "/definitions/blueprintCategories",
            Self::CategoryLists => "/declarations/blueprintCategoryLists",
        }
    }

    fn query(
        self,
        archive: &Archive,
        parsed: &ParsedCatalogQueries<'_>,
        origins: Option<&mut Origins>,
    ) -> TestResult<Option<Table>> {
        let all = CatalogRecordSelection::All;
        match self {
            Self::Global => parsed.with_crafting_global_params(all, None, |snapshot| {
                self.observe(
                    archive,
                    &Snapshot {
                        build: snapshot.build(),
                        availability: snapshot.availability(),
                        rows: snapshot.rows(),
                        coverage: snapshot.coverage(),
                        inventory: snapshot.source_inventory(),
                    },
                    |row| position(&row.source().guid(), row.source().record_index()),
                    origins,
                )
            })?,
            Self::Categories => {
                let snapshot = parsed.read_blueprint_categories(all, None)?;
                self.observe(
                    archive,
                    &Snapshot {
                        build: snapshot.build.as_ref(),
                        availability: snapshot.availability,
                        rows: &snapshot.rows,
                        coverage: &snapshot.coverage,
                        inventory: &snapshot.source_inventory,
                    },
                    |row| record_position(&row.source),
                    origins,
                )
            }
            Self::CategoryLists => {
                let snapshot = parsed.read_blueprint_category_lists(all, None)?;
                self.observe(
                    archive,
                    &Snapshot {
                        build: snapshot.build.as_ref(),
                        availability: snapshot.availability,
                        rows: &snapshot.rows,
                        coverage: &snapshot.coverage,
                        inventory: &snapshot.source_inventory,
                    },
                    |row| record_position(&row.source),
                    origins,
                )
            }
        }
    }

    fn observe<Row: Serialize>(
        self,
        archive: &Archive,
        snapshot: &Snapshot<'_, Row>,
        key: impl Fn(&Row) -> TestResult<(String, u32)>,
        origins: Option<&mut Origins>,
    ) -> TestResult<Option<Table>> {
        let &Snapshot {
            build,
            availability,
            rows,
            coverage,
            inventory,
        } = snapshot;
        let path = self.path();
        if build.is_none() || build != archive.build() {
            return Err(format!("{path}: crafting metadata query has a different Build").into());
        }
        component_family_parity::require_complete(path, availability, rows.len(), coverage)?;
        if coverage.skipped_leftovers != 0 {
            return Err(format!("{path}: unexpected crafting metadata policy exclusion").into());
        }
        require_canonical_order(rows, key)?;

        let uses = inventory.use_count();
        let sources = inventory.source_count();
        if !rows.is_empty() && (uses == 0 || sources == 0) {
            return Err(format!("{path}: crafting metadata rows lack original sources").into());
        }
        let table = origins
            .map(|origins| {
                real_archive_catalog_table_parity::capture_rows(
                    availability,
                    rows,
                    inventory,
                    origins,
                    wire_record_index,
                )
            })
            .transpose()?;
        let state = match (availability, rows.is_empty()) {
            (DatasetAvailability::Available, false) => "present",
            (DatasetAvailability::Available, true) => "complete-empty",
            (DatasetAvailability::Absent, true) => "absent-type",
            _ => return Err(format!("{path}: unresolved crafting metadata availability").into()),
        };
        print_line(&format!(
            "Crafting metadata SDK: dataset={path} availability={availability:?} rowEvidence={state} rows={} inventoryUses={uses} inventorySources={sources} skippedLeftovers={}",
            rows.len(),
            coverage.skipped_leftovers,
        ));
        Ok(table)
    }
}

fn record_position(source: &CatalogRecordSource) -> TestResult<(String, u32)> {
    position(&source.guid, &source.record_index)
}

fn position(
    guid: &SourcedValue<Guid>,
    record_index: &SourcedValue<u32>,
) -> TestResult<(String, u32)> {
    let guid = guid
        .value()
        .ok_or("missing crafting metadata GUID")?
        .to_string();
    let index = *record_index
        .value()
        .ok_or("missing crafting metadata Record index")?;
    Ok((guid, index))
}

fn require_canonical_order<Row>(
    rows: &[Row],
    key: impl Fn(&Row) -> TestResult<(String, u32)>,
) -> TestResult {
    let mut previous = None;
    let mut indices = HashSet::new();
    for row in rows {
        let current = key(row)?;
        if !indices.insert(current.1) || previous.is_some_and(|prior| prior >= current) {
            return Err(
                "crafting metadata repeats a physical Record or is out of canonical order".into(),
            );
        }
        previous = Some(current);
    }
    Ok(())
}

fn wire_record_index(row: &Value) -> TestResult<u32> {
    let index = row
        .get("recordIndex")
        .ok_or("missing crafting metadata Record index")?;
    if index.get("state").and_then(Value::as_str) != Some("present") {
        return Err("crafting metadata Record index must be present".into());
    }
    index
        .get("value")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| "crafting metadata Record index exceeds its wire width".into())
}

#[test]
#[ignore = "same Build: configure the installed Archive and exact completed Normal producer"]
fn installed_crafting_metadata_matches_one_complete_catalog() -> TestResult {
    let revision = env::var("P4KX_CRAFTING_METADATA_CATALOG_PRODUCER_REVISION")?;
    real_archive_catalog_table_parity::with_bound_measurement_from_producer(
        &revision,
        |archive, measurement| {
            let mut origins = Origins::default();
            let tables = {
                let session = CatalogQuerySession::open(
                    archive,
                    CatalogLanguageSelection::Selected(&[]),
                    None,
                )?;
                let parsed = session.parsed(None)?;
                let mut capture = |family: Family| -> TestResult<Table> {
                    family
                        .query(archive, &parsed, Some(&mut origins))?
                        .ok_or_else(|| {
                            format!(
                                "{}: missing complete crafting metadata table",
                                family.path()
                            )
                            .into()
                        })
                };
                let [global, categories, lists] = FAMILIES;
                [capture(global)?, capture(categories)?, capture(lists)?]
            };
            let counts =
                measurement.verify_tables(FAMILIES.map(Family::path), &tables, &origins)?;
            print_line(&format!(
                "complete crafting metadata SDK parity: rows={:?} sourceUses={} physicalSources={}",
                counts.rows, counts.source_uses, counts.physical_sources,
            ));
            Ok(())
        },
    )
}

#[test]
#[ignore = "standalone cold query: configure the installed Archive and one exact dataset path"]
fn installed_crafting_metadata_cold_query() -> TestResult {
    let path = env::var("P4KX_CRAFTING_METADATA_SDK_COLD_DATASET")?;
    let family = FAMILIES
        .into_iter()
        .find(|family| family.path() == path)
        .ok_or("configure one exact crafting metadata dataset path")?;
    equipment_input::with_archive(|archive| {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        let _ = family.query(archive, &parsed, None)?;
        Ok(())
    })
}

#[test]
fn crafting_metadata_requires_distinct_physical_records_in_canonical_order() -> TestResult {
    use serde_json::json;

    let early_guid = "00000000-0000-0000-0000-000000000001";
    let late_guid = "00000000-0000-0000-0000-000000000002";
    let key = |&(guid, index): &(&str, u32)| Ok((String::from(guid), index));
    require_canonical_order(&[(early_guid, 3), (early_guid, 7), (late_guid, 2)], key)?;
    assert!(
        require_canonical_order(&[(early_guid, 7), (early_guid, 3)], key).is_err(),
        "tied GUIDs must be ordered by physical Record index",
    );
    assert!(
        require_canonical_order(&[(early_guid, 7), (late_guid, 7)], key).is_err(),
        "different GUIDs cannot share a physical Record index",
    );
    assert_eq!(
        wire_record_index(&json!({"recordIndex": {"state": "present", "value": 7}}))?,
        7
    );
    assert!(
        wire_record_index(&json!({"recordIndex": {"state": "null", "value": 7}})).is_err(),
        "an unobserved identity cannot serve as the physical match key",
    );
    Ok(())
}
