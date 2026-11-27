// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, HashSet};

use p4kx_export::catalog_queries::{
    CanonicalRecordHeader, CanonicalText, CatalogRecordCoverage, DatasetAvailability, GameTextRef,
    PublicNameRef, ReputationRows,
};
use p4kx_extraction::{Archive, Build, Guid, Language, SourceInventory};
use serde::Serialize;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{
    BoundMeasurement, Counts, Origins, ScopedTextProof, Table, TestResult, capture_rows,
};
use crate::real_archive_verification::print_line;

use super::{COUNT, FAMILIES, Family};

pub(super) struct Proof<'scope> {
    archive: &'scope Archive,
    expected: [usize; COUNT],
    seen: BTreeSet<Family>,
    retained: Option<Retained>,
}

#[derive(Default)]
struct Retained {
    tables: BTreeMap<Family, Table>,
    texts: ScopedTextProof,
    origins: Origins,
}

impl<'scope> Proof<'scope> {
    pub(super) const fn cold(archive: &'scope Archive, expected: [usize; COUNT]) -> Self {
        Self {
            archive,
            expected,
            seen: BTreeSet::new(),
            retained: None,
        }
    }

    pub(super) fn retained(archive: &'scope Archive, expected: [usize; COUNT]) -> Self {
        Self {
            retained: Some(Retained::default()),
            ..Self::cold(archive, expected)
        }
    }

    pub(super) fn require_build(&self, build: Option<&Build>) -> TestResult {
        if build.is_none() || build != self.archive.build() {
            return Err("a Reputation or Faction query is not bound to the installed Build".into());
        }
        Ok(())
    }

    pub(super) fn observe<Row: Serialize>(
        &mut self,
        family: Family,
        observed: &Snapshot<'_, Row>,
        position: impl Fn(&Row) -> TestResult<Position>,
    ) -> TestResult {
        let path = family.path();
        let [uses, sources] = [
            observed.inventory.use_count(),
            observed.inventory.source_count(),
        ];
        let count = observed.rows.len();
        verify_coverage(family, observed, self.expected(family)?)?;
        verify_order(observed.rows, position)?;
        if !self.seen.insert(family) {
            return Err(format!("{path}: route was observed more than once").into());
        }
        if let Some(retained) = &mut self.retained {
            let table = capture_rows(
                observed.availability,
                observed.rows,
                observed.inventory,
                &mut retained.origins,
                |row| wire_position(family, row),
            )?;
            retained.tables.insert(family, table);
        }
        print_line(&format!(
            "Reputation SDK: dataset={path} availability={:?} rows={count} inventoryUses={uses} inventorySources={sources} skippedLeftovers={}",
            observed.availability, observed.coverage.skipped_leftovers,
        ));
        Ok(())
    }

    pub(super) fn observe_texts(
        &mut self,
        texts: &Localized<'_>,
        inventory: &SourceInventory,
    ) -> TestResult {
        if texts
            .available
            .iter()
            .any(|language| !texts.requested.contains(language))
            || (!texts.public.is_empty() || !texts.protected.is_empty())
                && texts.requested.is_empty()
        {
            return Err(
                "the selected Reputation or Faction text has an inconsistent locale policy".into(),
            );
        }
        if let Some(retained) = &mut self.retained {
            retained.texts.retain(
                texts.public,
                texts.protected,
                inventory,
                &mut retained.origins,
            )?;
        } else {
            let mut wires = Vec::new();
            for row in texts.public {
                wires.push(serde_json::to_value(row)?);
            }
            for row in texts.protected {
                wires.push(serde_json::to_value(row)?);
            }
            if !wires.is_empty() {
                Origins::default().extend(inventory, &wires)?;
            }
        }
        Ok(())
    }

    fn expected(&self, family: Family) -> TestResult<usize> {
        self.expected
            .get(family as usize)
            .copied()
            .ok_or_else(|| "missing Reputation route count".into())
    }

    pub(super) fn finish_cold(self, family: Family) -> TestResult {
        if self.seen != BTreeSet::from([family]) {
            return Err("the cold proof differs from its exact single-route scope".into());
        }
        Ok(())
    }

    pub(super) fn compare(self, measurement: &BoundMeasurement) -> TestResult<Counts<COUNT>> {
        let Some(Retained {
            mut tables,
            texts,
            origins,
        }) = self.retained
        else {
            return Err("the retained proof was not captured".into());
        };
        let tables: [Table; COUNT] = FAMILIES
            .into_iter()
            .map(|family| {
                tables
                    .remove(&family)
                    .ok_or("missing selected Reputation or Faction route")
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "the retained Reputation routes are incomplete")?;
        measurement.verify_table_scope(
            FAMILIES.map(Family::path),
            &tables,
            &[],
            &origins,
            Some(&texts),
        )
    }
}

pub(super) struct Localized<'read> {
    pub(super) requested: &'read [Language],
    pub(super) available: &'read [Language],
    pub(super) public: &'read [CanonicalText<'read, PublicNameRef>],
    pub(super) protected: &'read [CanonicalText<'read, GameTextRef>],
}

pub(super) struct Snapshot<'read, Row> {
    pub(super) rows: &'read [Row],
    pub(super) availability: DatasetAvailability,
    pub(super) coverage: &'read CatalogRecordCoverage,
    pub(super) inventory: &'read SourceInventory,
}

impl<'read, Row> Snapshot<'read, Row> {
    pub(super) fn reputation(
        rows: &'read ReputationRows<Row>,
        inventory: &'read SourceInventory,
    ) -> Self {
        Self {
            rows: &rows.rows,
            availability: rows.availability,
            coverage: &rows.coverage,
            inventory,
        }
    }
}

fn verify_coverage<Row>(
    family: Family,
    observed: &Snapshot<'_, Row>,
    expected: usize,
) -> TestResult {
    let count = observed.rows.len();
    let coverage = observed.coverage;
    if count != expected
        || !matches!(
            (observed.availability, count),
            (DatasetAvailability::Available, _) | (DatasetAvailability::Absent, 0)
        )
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
        || coverage.skipped_leftovers != 0
        || (count != 0
            && (observed.inventory.source_count() == 0 || observed.inventory.use_count() == 0))
    {
        return Err(format!(
            "{}: expected {expected} rows with complete original coverage, observed {count} {:?}, unreadableHeaders={} unreadableBodies={} incompleteFields={} skippedLeftovers={}",
            family.path(), observed.availability, coverage.unreadable_headers.len(),
            coverage.unreadable_bodies.len(), coverage.incomplete_fields.len(), coverage.skipped_leftovers,
        ).into());
    }
    Ok(())
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Position {
    guid: String,
    record_index: u32,
}

impl Position {
    pub(super) fn new(guid: Option<&Guid>, index: Option<&u32>) -> TestResult<Self> {
        Ok(Self {
            guid: guid
                .ok_or("the typed row lacks its physical Record GUID")?
                .to_string(),
            record_index: *index.ok_or("the typed row lacks its physical Record index")?,
        })
    }

    pub(super) fn record(header: &CanonicalRecordHeader<'_>) -> TestResult<Self> {
        Self::new(header.guid().value(), header.record_index().value())
    }
}

fn verify_order<Row>(rows: &[Row], position: impl Fn(&Row) -> TestResult<Position>) -> TestResult {
    let mut indices = HashSet::new();
    let mut previous = None;
    for row in rows {
        let actual = position(row)?;
        if !indices.insert(actual.record_index) || previous.is_some_and(|prior| actual <= prior) {
            return Err("a row repeats its physical Record or disagrees with canonical GUID and index order".into());
        }
        previous = Some(actual);
    }
    Ok(())
}

fn wire_position(family: Family, row: &Value) -> TestResult<u32> {
    let pointer = if family == Family::Triggers {
        "/owner/recordIndex"
    } else {
        "/recordIndex"
    };
    let index = row
        .pointer(pointer)
        .ok_or("missing exact physical Reputation or Faction identity")?;
    if index.get("state").and_then(Value::as_str) != Some("present") {
        return Err("the physical Reputation or Faction row identity is unreadable".into());
    }
    index
        .get("value")
        .and_then(Value::as_u64)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(|| "invalid physical Reputation or Faction row index".into())
}
