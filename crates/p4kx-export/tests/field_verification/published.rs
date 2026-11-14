// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::Guid;
use serde_json::Value;

use super::compare::{self, Class, Summary};
use super::error::{self, Error, Kind};
use super::inventory::FieldKey;
use super::joins::{self, Index, Multiplicity};
use super::path::Path;
use super::published_io::Dataset;
use super::published_model::{Build, BuildGate, Family, Field, Publication, gate};

pub(crate) struct Report {
    pub(crate) gate: BuildGate,
    pub(crate) fields: BTreeMap<FieldKey, Summary>,
    pub(crate) matched_guids: u64,
    pub(crate) reference_only_guids: u64,
    pub(crate) catalog_only_guids: u64,
    pub(crate) excluded_guids: u64,
    pub(crate) missing_both_guids: u64,
}

impl Report {
    pub(crate) fn succeeds(&self) -> bool {
        self.gate == BuildGate::SamePublishedBuild
            && self.missing_both_guids == 0
            && self.fields.values().map(Summary::compared).sum::<u64>() > 0
            && self.fields.values().all(Summary::clean)
    }
}

fn reference<'a>(
    dataset: &'a Dataset,
    family: Family,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Index<'a>> {
    let mut combined = Index::new();
    let multiplicity = if family == Family::Placements {
        Multiplicity::Many
    } else {
        Multiplicity::Unique
    };
    for file in dataset
        .snapshot
        .files
        .iter()
        .filter(|file| file.family == family)
    {
        let key = p4kx_testkit::offline_seam::relative_path(&file.path, 4096)?.to_lowercase();
        let document = dataset
            .documents
            .get(&key)
            .ok_or_else(|| Error::new(Kind::Input, "selected reference document absent"))?;
        let rows = file.rows.select(document, cancelled)?;
        for (guid, rows) in joins::index(rows, &file.identity, multiplicity, cancelled)? {
            let existing = combined.entry(guid).or_default();
            if multiplicity == Multiplicity::Unique && !existing.is_empty() {
                return Err(Error::new(
                    Kind::Identity,
                    "duplicate GUID across selected reference artifacts",
                )
                .into());
            }
            existing.extend(rows);
        }
    }
    Ok(combined)
}

fn catalog<'a>(
    catalog: &'a Value,
    family: Family,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Index<'a>> {
    let rows = catalog
        .pointer(family.rows_pointer())
        .and_then(Value::as_array);
    joins::index(
        rows.into_iter().flatten(),
        &Path::fields(family.identity_fields()),
        if family == Family::Placements {
            Multiplicity::Many
        } else {
            Multiplicity::Unique
        },
        cancelled,
    )
}

fn relationship<'a>(
    index: &Index<'a>,
    path: &Path,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Index<'a>> {
    joins::index(
        index.values().flatten().copied(),
        path,
        Multiplicity::Many,
        cancelled,
    )
}

const fn missing_rows(reference: bool, catalog: bool) -> Option<Class> {
    match (reference, catalog) {
        (true, false) => Some(Class::ReferenceOnly),
        (false, true) => Some(Class::P4kxOnly),
        (false, false) => Some(Class::Missing),
        (true, true) => None,
    }
}

fn compare_rows(
    reference: Option<&Vec<&Value>>,
    catalog: Option<&Vec<&Value>>,
    field: &Field,
    cancelled: &impl Fn() -> bool,
    summary: &mut Summary,
) -> super::Result<()> {
    if let Some(class) = missing_rows(reference.is_some(), catalog.is_some()) {
        summary.record(class);
        return Ok(());
    }
    let mut left = Vec::new();
    let mut right = Vec::new();
    let reference_rows = reference.into_iter().flatten();
    for row in reference_rows {
        left.extend(field.reference.select(row, cancelled)?);
    }
    let catalog_rows = catalog.into_iter().flatten();
    for row in catalog_rows {
        right.extend(field.catalog.select(row, cancelled)?);
    }
    let broadcast = field.resource_join.is_some();
    if broadcast
        && reference.is_some_and(|rows| rows.len() > 1)
        && catalog.is_some_and(|rows| rows.len() > 1)
    {
        for _ in 0..left.len().max(right.len()).max(1) {
            summary.record(Class::Uncomparable);
        }
        return Ok(());
    }
    for index in 0..left.len().max(right.len()).max(1) {
        error::check(cancelled)?;
        summary.record(compare::values(
            left.get(if broadcast && left.len() == 1 {
                0
            } else {
                index
            })
            .copied(),
            right
                .get(if broadcast && right.len() == 1 {
                    0
                } else {
                    index
                })
                .copied(),
            &field.reference_unit,
            &field.catalog_unit,
            &field.rule,
        ));
    }
    Ok(())
}

fn membership(
    report: &mut Report,
    left: &Index<'_>,
    right: &Index<'_>,
    selected: &BTreeSet<Guid>,
    excluded: &BTreeSet<Guid>,
) {
    for guid in joins::union(left, right) {
        if excluded.contains(&guid) || (!selected.is_empty() && !selected.contains(&guid)) {
            report.excluded_guids += 1;
        } else {
            match (left.contains_key(&guid), right.contains_key(&guid)) {
                (true, true) => report.matched_guids += 1,
                (true, false) => report.reference_only_guids += 1,
                (false, true) => report.catalog_only_guids += 1,
                (false, false) => {}
            }
        }
    }
}

fn filtered<'a>(
    index: Index<'a>,
    selected: &BTreeSet<Guid>,
    excluded: &BTreeSet<Guid>,
) -> Index<'a> {
    index
        .into_iter()
        .filter(|(guid, _)| {
            !excluded.contains(guid) && (selected.is_empty() || selected.contains(guid))
        })
        .collect()
}

fn scope(dataset: &Dataset) -> super::Result<(BTreeSet<Guid>, BTreeSet<Guid>)> {
    let selected = dataset
        .snapshot
        .selection
        .iter()
        .map(|guid| joins::guid(guid))
        .collect::<super::Result<BTreeSet<_>>>()?;
    let excluded = dataset
        .snapshot
        .exclusions
        .iter()
        .map(|entry| joins::guid(&entry.guid))
        .collect::<super::Result<BTreeSet<_>>>()?;
    Ok((selected, excluded))
}

pub(crate) fn verify(
    dataset: &Dataset,
    catalog_value: &Value,
    actual: &Build,
    publications: &[Publication],
    cancelled: &impl Fn() -> bool,
) -> super::Result<Report> {
    let gate = gate(&dataset.snapshot.provenance, actual, publications);
    let mut report = Report {
        gate,
        fields: BTreeMap::new(),
        matched_guids: 0,
        reference_only_guids: 0,
        catalog_only_guids: 0,
        excluded_guids: 0,
        missing_both_guids: 0,
    };
    if gate != BuildGate::SamePublishedBuild {
        return Ok(report);
    }
    let (selected, excluded) = scope(dataset)?;
    let mut counted = BTreeSet::new();
    let mut observed_guids = BTreeSet::new();
    for field in &dataset.snapshot.fields {
        error::check(cancelled)?;
        let left = reference(dataset, field.family, cancelled)?;
        let right = catalog(catalog_value, field.family, cancelled)?;
        if counted.insert(field.family) {
            observed_guids.extend(joins::union(&left, &right));
            membership(&mut report, &left, &right, &selected, &excluded);
        }
        let (left, right) = if let Some(join) = &field.resource_join {
            (
                relationship(
                    &filtered(left, &selected, &excluded),
                    &join.reference,
                    cancelled,
                )?,
                relationship(
                    &filtered(right, &selected, &excluded),
                    &join.catalog,
                    cancelled,
                )?,
            )
        } else {
            (left, right)
        };
        let identities = joins::union(&left, &right);
        let summary = report.fields.entry(field.source.clone()).or_default();
        for guid in identities {
            if field.resource_join.is_none()
                && (excluded.contains(&guid) || (!selected.is_empty() && !selected.contains(&guid)))
            {
                summary.record(Class::Excluded);
                continue;
            }
            compare_rows(left.get(&guid), right.get(&guid), field, cancelled, summary)?;
        }
    }
    report.missing_both_guids = selected.difference(&observed_guids).count() as u64;
    dataset.unchanged(cancelled)?;
    Ok(report)
}
