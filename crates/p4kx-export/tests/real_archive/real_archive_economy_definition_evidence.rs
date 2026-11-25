// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::Path;

use p4kx_export::catalog_queries::commodities::CommoditySnapshot;
use p4kx_export::catalog_queries::{CatalogRecordCoverage, DatasetAvailability};
use p4kx_extraction::{Archive, Build, Language, SourceInventory};
use serde::Serialize;

use crate::real_archive_catalog_table_parity::{
    BoundMeasurement, Counts, DefinitionOwner, StreamedDefinitionDigests, TestResult,
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
    retained: Option<StreamedDefinitionDigests<'archive, COUNT>>,
}

impl<'archive> Evidence<'archive> {
    pub(super) const fn cold(archive: &'archive Archive, expected: Census) -> Self {
        Self {
            archive,
            expected,
            retained: None,
        }
    }

    pub(super) fn retained(
        archive: &'archive Archive,
        expected: Census,
        directory: &'archive Path,
    ) -> Self {
        Self {
            retained: Some(StreamedDefinitionDigests::new(
                directory,
                FAMILIES.map(|family| match family {
                    Family::CommodityPresentation => DefinitionOwner::CommodityPresentation,
                    _ => DefinitionOwner::Record,
                }),
            )),
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
        self.observe(family, scope, rows)
    }

    pub(super) fn presentation(
        &mut self,
        scope: &RecordScope<'_>,
        snapshot: &CommoditySnapshot<'_>,
    ) -> TestResult {
        let mut record_indices = HashSet::new();
        let mut header_uses = HashSet::new();
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
                || !record_indices.insert(*record)
                || !header_uses.insert(reference.as_str())
            {
                return Err(
                    "the Commodity definition and presentation use different physical GUID headers"
                        .into(),
                );
            }
        }
        if record_indices.len() != snapshot.presentations.len()
            || record_indices.len() != snapshot.definitions.len()
        {
            return Err("a Commodity definition or presentation lacks its paired row".into());
        }
        self.observe(
            Family::CommodityPresentation,
            scope,
            &snapshot.presentations,
        )
    }

    fn observe<Row: Serialize>(
        &mut self,
        family: Family,
        scope: &RecordScope<'_>,
        rows: &[Row],
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
            retained.retain(family as usize, scope.availability, rows, scope.inventory)?;
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
            retained.retain_texts(public, protected, inventory)?;
        }
        Ok(())
    }

    pub(super) fn compare(self, measurement: &BoundMeasurement) -> TestResult<Counts<COUNT>> {
        self.retained
            .ok_or_else(|| "the retained economy proof was not captured".into())
            .and_then(|retained| retained.compare(measurement, FAMILIES.map(Family::path)))
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
