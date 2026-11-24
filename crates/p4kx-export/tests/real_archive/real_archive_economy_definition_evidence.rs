// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap, HashSet};

use p4kx_export::catalog_queries::commodities::CommoditySnapshot;
use p4kx_export::catalog_queries::{CatalogRecordCoverage, DatasetAvailability};
use p4kx_extraction::{Archive, Build, Language, SourceInventory};
use serde::Serialize;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{
    BoundMeasurement, Counts, Origins, ScopedTextProof, Table, TestResult, capture_rows,
};
use crate::real_archive_verification::print_line;

use super::{COUNT, FAMILIES, Family};

pub(super) struct Census([Expected; COUNT]);

struct Expected {
    rows: usize,
    skipped: usize,
}

impl Census {
    pub(super) fn parse(config: &str) -> TestResult<Self> {
        let expected = config
            .split(',')
            .map(|slot| {
                let (rows, skipped) = slot.split_once(':').ok_or("expected rows:skipped")?;
                Ok(Expected {
                    rows: rows.parse()?,
                    skipped: skipped.parse()?,
                })
            })
            .collect::<TestResult<Vec<_>>>()?;
        Ok(Self(
            expected
                .try_into()
                .map_err(|_| "expected seven route counts")?,
        ))
    }

    fn get(&self, family: Family) -> TestResult<&Expected> {
        self.0
            .get(family as usize)
            .ok_or_else(|| "missing installed route census".into())
    }
}

pub(super) struct RecordScope<'read> {
    pub(super) build: Option<&'read Build>,
    pub(super) availability: DatasetAvailability,
    pub(super) inventory: &'read SourceInventory,
    pub(super) coverage: &'read CatalogRecordCoverage,
}

pub(super) struct Evidence<'archive> {
    archive: &'archive Archive,
    expected: Census,
    retained: Option<Retained>,
}

#[derive(Default)]
struct Retained {
    tables: BTreeMap<Family, Table>,
    origins: Origins,
    texts: ScopedTextProof,
}

impl<'archive> Evidence<'archive> {
    pub(super) const fn cold(archive: &'archive Archive, expected: Census) -> Self {
        Self {
            archive,
            expected,
            retained: None,
        }
    }

    pub(super) fn retained(archive: &'archive Archive, expected: Census) -> Self {
        Self {
            retained: Some(Retained::default()),
            ..Self::cold(archive, expected)
        }
    }

    pub(super) fn records<Row: Serialize>(
        &mut self,
        family: Family,
        scope: &RecordScope<'_>,
        rows: &[Row],
        index: impl Fn(&Row) -> Option<u32>,
    ) -> TestResult {
        let mut seen = HashSet::new();
        for row in rows {
            let index = index(row).ok_or("a typed row lacks its physical Record index")?;
            if !seen.insert(index) {
                return Err(format!("{} repeats a physical Record index", family.path()).into());
            }
        }
        self.observe(family, scope, rows, physical_record)
    }

    pub(super) fn presentation(
        &mut self,
        scope: &RecordScope<'_>,
        snapshot: &CommoditySnapshot<'_>,
    ) -> TestResult {
        let mut physical = HashMap::new();
        for (definition, presentation) in snapshot.definitions.iter().zip(&snapshot.presentations) {
            let header = definition.source();
            let original = header.guid();
            let projected = presentation.commodity_guid();
            let record = header
                .record_index()
                .value()
                .ok_or("the Commodity owner is not physical")?;
            let [reference] = projected.origin().source_uses_refs() else {
                return Err("the Commodity GUID lacks one original header use".into());
            };
            if original.value().is_none()
                || original.value().copied() != projected.value().map(|guid| guid.get())
                || original.origin() != projected.origin()
                || !projected.origin().source_set_complete()
                || physical.insert(reference.as_str(), *record).is_some()
            {
                return Err(
                    "the Commodity definition and presentation use different physical GUID headers"
                        .into(),
                );
            }
        }
        if physical.len() != snapshot.presentations.len()
            || physical.len() != snapshot.definitions.len()
        {
            return Err("a Commodity definition or presentation lacks its paired row".into());
        }
        self.observe(
            Family::CommodityPresentation,
            scope,
            &snapshot.presentations,
            |row| {
                physical
                    .get(presentation_source(row)?)
                    .copied()
                    .ok_or_else(|| "no matching original Commodity GUID header use".into())
            },
        )
    }

    fn observe<Row: Serialize>(
        &mut self,
        family: Family,
        scope: &RecordScope<'_>,
        rows: &[Row],
        physical: impl Fn(&Value) -> TestResult<u32>,
    ) -> TestResult {
        let path = family.path();
        let count = rows.len();
        let expected = self.expected.get(family)?;
        let uses = scope.inventory.use_count();
        let sources = scope.inventory.source_count();
        require_readable(path, scope.coverage)?;
        if scope.build.is_none()
            || scope.build != self.archive.build()
            || !known(scope.availability, count)
            || count != expected.rows
            || scope.coverage.skipped_leftovers != expected.skipped
            || (count != 0 && (uses == 0 || sources == 0))
        {
            return Err(format!(
                "{path}: expected matching Build, {} rows, {} skipped with original evidence, observed {count} {:?}, skipped={} uses={uses} sources={sources}",
                expected.rows, expected.skipped, scope.availability, scope.coverage.skipped_leftovers,
            ).into());
        }
        if let Some(retained) = &mut self.retained {
            let table = capture_rows(
                scope.availability,
                rows,
                scope.inventory,
                &mut retained.origins,
                physical,
            )?;
            if retained.tables.insert(family, table).is_some() {
                return Err(format!("{path}: SDK dataset repeated").into());
            }
        }
        print_line(&format!(
            "Economy definition SDK: dataset={path} availability={:?} rows={count} inventoryUses={uses} inventorySources={sources} skippedLeftovers={}",
            scope.availability, scope.coverage.skipped_leftovers,
        ));
        Ok(())
    }

    pub(super) fn texts(
        &mut self,
        public: &[impl Serialize],
        protected: &[impl Serialize],
        inventory: &SourceInventory,
        requested: &[Language],
        available: &[Language],
    ) -> TestResult {
        if available
            .iter()
            .any(|language| !requested.contains(language))
            || (!public.is_empty() || !protected.is_empty()) && requested.is_empty()
        {
            return Err("the localized economy query has inconsistent language scope".into());
        }
        if let Some(retained) = &mut self.retained {
            retained
                .texts
                .retain(public, protected, inventory, &mut retained.origins)?;
        }
        Ok(())
    }

    pub(super) fn compare(self, measurement: &BoundMeasurement) -> TestResult<Counts<COUNT>> {
        let Some(Retained {
            mut tables,
            origins,
            texts,
        }) = self.retained
        else {
            return Err("the retained economy proof was not captured".into());
        };
        let tables: [Table; COUNT] = FAMILIES
            .into_iter()
            .map(|family| {
                tables
                    .remove(&family)
                    .ok_or("missing selected economy route")
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "the retained economy routes are incomplete")?;
        measurement.verify_table_scope(
            FAMILIES.map(Family::path),
            &tables,
            &[],
            &origins,
            Some(&texts),
        )
    }
}

pub(super) fn require_readable(path: &str, coverage: &CatalogRecordCoverage) -> TestResult {
    if !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
    {
        return Err(format!(
            "{path}: unreadableHeaders={} unreadableBodies={} incompleteFields={}",
            coverage.unreadable_headers.len(),
            coverage.unreadable_bodies.len(),
            coverage.incomplete_fields.len(),
        )
        .into());
    }
    Ok(())
}

const fn known(availability: DatasetAvailability, count: usize) -> bool {
    matches!(
        (availability, count),
        (DatasetAvailability::Available, _) | (DatasetAvailability::Absent, 0)
    )
}

fn physical_record(row: &Value) -> TestResult<u32> {
    if row.pointer("/recordIndex/state").and_then(Value::as_str) != Some("present") {
        return Err("the serialized Record index is unreadable".into());
    }
    row.pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(|| "the serialized physical Record index is invalid".into())
}

fn presentation_source(row: &Value) -> TestResult<&str> {
    let guid = row
        .get("commodityGuid")
        .ok_or("the Commodity presentation lacks its identity")?;
    let Some([reference]) = guid
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return Err(
            "the Commodity presentation identity does not have one original header use".into(),
        );
    };
    reference
        .as_str()
        .ok_or_else(|| "the Commodity presentation header reference is invalid".into())
}

#[test]
fn commodity_presentation_identity_uses_the_physical_header() -> TestResult {
    use serde_json::json;

    let first = json!({"commodityGuid":{"value":"same","sourceUsesRefs":["first"]}});
    let second = json!({"commodityGuid":{"value":"same","sourceUsesRefs":["second"]}});
    let ambiguous = json!({"commodityGuid":{"value":"same","sourceUsesRefs":["first","second"]}});
    assert_ne!(
        presentation_source(&first)?,
        presentation_source(&second)?,
        "nominal duplicates have distinct owners"
    );
    assert!(
        presentation_source(&ambiguous).is_err(),
        "a single direct GUID header use is required"
    );
    assert!(
        presentation_source(&json!({"commodityGuid":{"value":"same"}})).is_err(),
        "a GUID value alone cannot claim an original owner"
    );
    Ok(())
}
