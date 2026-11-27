// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::Path;

use p4kx_export::catalog_queries::starmap::StarmapDefinitionSnapshot;
use p4kx_export::catalog_queries::{
    CatalogRecordCoverage, DatasetAvailability, LooseDefinitionSnapshot,
};
use p4kx_extraction::{Archive, Build, SourceInventory};
use serde::Serialize;

use crate::real_archive_catalog_table_parity::{
    BoundMeasurement, Counts, DefinitionOwner, StreamedDefinitionDigests, TestResult,
};
use crate::real_archive_verification::print_line;

use super::{COUNT, FAMILIES, Family};

pub(super) struct Expected {
    rows: [usize; COUNT],
    object_leftovers: usize,
}

impl Expected {
    pub(super) fn parse(rows: &str, object_leftovers: &str) -> TestResult<Self> {
        let rows = rows
            .split(',')
            .map(str::parse::<usize>)
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "configure exactly eight World definition row counts")?;
        Ok(Self {
            rows,
            object_leftovers: object_leftovers.parse()?,
        })
    }

    const fn leftovers(&self, family: Family) -> usize {
        match family {
            Family::StarmapObjects => self.object_leftovers,
            _ => 0,
        }
    }
}

pub(super) struct RecordScope<'read> {
    pub(super) build: Option<&'read Build>,
    pub(super) availability: DatasetAvailability,
    pub(super) inventory: &'read SourceInventory,
    pub(super) coverage: &'read CatalogRecordCoverage,
}

pub(super) struct Proof<'archive> {
    archive: &'archive Archive,
    expected: Expected,
    retained: Option<StreamedDefinitionDigests<'archive, COUNT>>,
}

impl<'archive> Proof<'archive> {
    pub(super) const fn cold(archive: &'archive Archive, expected: Expected) -> Self {
        Self {
            archive,
            expected,
            retained: None,
        }
    }

    pub(super) fn retained(
        archive: &'archive Archive,
        expected: Expected,
        directory: &'archive Path,
    ) -> Self {
        Self {
            retained: Some(StreamedDefinitionDigests::new(
                directory,
                FAMILIES.map(|family| match family {
                    Family::Soils | Family::SoilTextureSets | Family::PlanetResources => {
                        DefinitionOwner::Entry
                    }
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
        identity: impl Fn(&Row) -> Option<u32>,
    ) -> TestResult {
        self.check_build(scope.build)?;
        let path = family.path();
        let coverage = scope.coverage;
        if !coverage.unreadable_headers.is_empty()
            || !coverage.unreadable_bodies.is_empty()
            || !coverage.incomplete_fields.is_empty()
        {
            return Err(format!(
                "{path}: incomplete Record evidence, unreadableHeaders={} unreadableBodies={} incompleteFields={}",
                coverage.unreadable_headers.len(),
                coverage.unreadable_bodies.len(),
                coverage.incomplete_fields.len(),
            )
            .into());
        }
        self.observe(
            &Observation {
                family,
                rows,
                availability: scope.availability,
                inventory: scope.inventory,
                skipped: coverage.skipped_leftovers,
            },
            identity,
        )
    }

    pub(super) fn loose<Row: Serialize>(
        &mut self,
        family: Family,
        snapshot: &LooseDefinitionSnapshot<Row>,
        identity: impl Fn(&Row) -> u32,
    ) -> TestResult {
        self.check_build(snapshot.build.as_ref())?;
        if !snapshot.is_selection_complete() || snapshot.entries.failed_dependencies != 0 {
            return Err(format!(
                "{}: incomplete Archive Entry selection or dependency evidence: {:?}",
                family.path(),
                snapshot.entries,
            )
            .into());
        }
        self.observe(
            &Observation {
                family,
                rows: &snapshot.rows,
                availability: snapshot.availability,
                inventory: &snapshot.source_inventory,
                skipped: snapshot.entries.intentionally_skipped,
            },
            |row| Some(identity(row)),
        )
    }

    pub(super) fn texts(&mut self, snapshot: &StarmapDefinitionSnapshot<'_>) -> TestResult {
        if snapshot
            .available_languages
            .iter()
            .any(|language| !snapshot.requested_languages.contains(language))
            || (!snapshot.public_names.is_empty() || !snapshot.game_texts.is_empty())
                && snapshot.requested_languages.is_empty()
        {
            return Err("the Starmap query has inconsistent localization coverage".into());
        }
        if let Some(retained) = &mut self.retained {
            retained.retain_texts(
                &snapshot.public_names,
                &snapshot.game_texts,
                &snapshot.source_inventory,
            )?;
        }
        Ok(())
    }

    fn check_build(&self, build: Option<&Build>) -> TestResult {
        if build.is_none() || build != self.archive.build() {
            return Err("the World definition query did not use the installed Build".into());
        }
        Ok(())
    }

    fn observe<Row: Serialize>(
        &mut self,
        observed: &Observation<'_, Row>,
        identity: impl Fn(&Row) -> Option<u32>,
    ) -> TestResult {
        let family = observed.family;
        let path = family.path();
        let count = observed.rows.len();
        let expected = *self
            .expected
            .rows
            .get(family as usize)
            .ok_or("missing route count")?;
        let expected_skipped = self.expected.leftovers(family);
        let uses = observed.inventory.use_count();
        let sources = observed.inventory.source_count();
        if !known_disposition(observed.availability, count)
            || count != expected
            || observed.skipped != expected_skipped
            || (count != 0 && (uses == 0 || sources == 0))
        {
            return Err(format!(
                "{path}: expected {expected} rows and {expected_skipped} leftovers, observed {count} {:?}, leftovers={}, inventoryUses={uses} inventorySources={sources}",
                observed.availability, observed.skipped,
            )
            .into());
        }
        require_unique(observed.rows.iter().map(identity))?;
        if let Some(retained) = &mut self.retained {
            retained.retain(
                family as usize,
                observed.availability,
                observed.rows,
                observed.inventory,
            )?;
        }
        print_line(&format!(
            "World definition SDK: dataset={path} availability={:?} rows={count} inventoryUses={uses} inventorySources={sources} skippedLeftovers={}",
            observed.availability, observed.skipped,
        ));
        Ok(())
    }

    pub(super) fn compare(self, measurement: &BoundMeasurement) -> TestResult<Counts<COUNT>> {
        self.retained
            .ok_or_else(|| "the retained World definition proof was not captured".into())
            .and_then(|retained| retained.compare(measurement, FAMILIES.map(Family::path)))
    }
}

struct Observation<'read, Row> {
    family: Family,
    rows: &'read [Row],
    availability: DatasetAvailability,
    inventory: &'read SourceInventory,
    skipped: usize,
}

const fn known_disposition(availability: DatasetAvailability, count: usize) -> bool {
    matches!(
        (availability, count),
        (DatasetAvailability::Available, _) | (DatasetAvailability::Absent, 0)
    )
}

fn require_unique(indices: impl IntoIterator<Item = Option<u32>>) -> TestResult {
    let mut positions = HashSet::new();
    for index in indices {
        let index = index.ok_or("the typed row omits its physical owner index")?;
        if !positions.insert(index) {
            return Err("the typed rows repeat a physical owner index".into());
        }
    }
    Ok(())
}

#[test]
fn distinguishes_physical_owner_domains_and_absence() -> TestResult {
    use serde_json::json;

    let record = json!({"recordIndex":{"state":"present","value":4}});
    let entry = json!({"identity":{"entryIndex":4}});
    assert_eq!(
        DefinitionOwner::Record.physical_index(&record)?,
        4,
        "original Record"
    );
    assert_eq!(
        DefinitionOwner::Entry.physical_index(&entry)?,
        4,
        "original Entry"
    );
    assert!(
        DefinitionOwner::Record.physical_index(&entry).is_err(),
        "Entry is no Record"
    );
    assert!(
        DefinitionOwner::Entry.physical_index(&record).is_err(),
        "Record is no Entry"
    );
    assert!(
        require_unique([Some(4), Some(4)]).is_err(),
        "physical owner repeated"
    );
    assert!(require_unique([None]).is_err(), "physical owner missing");
    assert!(
        known_disposition(DatasetAvailability::Absent, 0),
        "observed absence"
    );
    assert!(
        known_disposition(DatasetAvailability::Available, 0),
        "declared empty"
    );
    assert!(
        !known_disposition(DatasetAvailability::Absent, 1),
        "absence with a row"
    );
    assert!(
        !known_disposition(DatasetAvailability::Unavailable, 0),
        "unknown ownership"
    );
    Ok(())
}
