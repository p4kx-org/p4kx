// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::Guid;
use serde_json::Value;

use super::inventory::{Disposition, FieldKey, Inventory};
use super::joins::Index;
use super::schema::Schema;
use super::source::Observations;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Review {
    #[default]
    Unreviewed,
    Reviewed,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    #[default]
    Stable,
    Changed,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Population {
    #[default]
    Stable,
    NewlyPopulated,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Counts {
    pub(crate) declared: bool,
    pub(crate) array: bool,
    pub(crate) review: Review,
    pub(crate) present: u64,
    pub(crate) populated: u64,
    pub(crate) exported: u64,
    pub(crate) unhandled: u64,
    pub(crate) rejected: u64,
    pub(crate) read_errors: u64,
    pub(crate) missing_targets: u64,
    pub(crate) missing_export_guid: u64,
    pub(crate) excluded: u64,
    pub(crate) target: Target,
    pub(crate) population: Population,
}

pub(crate) struct Projection<'a> {
    pub(crate) groups: BTreeMap<String, Index<'a>>,
    pub(crate) rejected: std::collections::BTreeSet<Guid>,
    pub(crate) exclusions: BTreeMap<Guid, String>,
}

const fn populated(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Array(values) => !values.is_empty(),
        Value::String(value) => !value.is_empty(),
        _ => true,
    }
}

fn declared(
    declaration: &super::schema::Declaration,
    mapping: Option<&super::inventory::Mapping>,
) -> Counts {
    Counts {
        declared: true,
        array: declaration.array,
        review: if mapping.is_some() {
            Review::Reviewed
        } else {
            Review::Unreviewed
        },
        target: if mapping.is_some_and(|mapping| mapping.declared_target != declaration.target) {
            Target::Changed
        } else {
            Target::Stable
        },
        ..Counts::default()
    }
}

pub(crate) fn account(
    schema: &Schema,
    observations: &Observations,
    inventory: &Inventory,
    projection: &Projection<'_>,
    cancelled: &impl Fn() -> bool,
) -> super::Result<BTreeMap<FieldKey, Counts>> {
    let mappings = inventory.reviewed()?;
    let mut counts = BTreeMap::new();
    for (key, declaration) in &schema.declarations {
        counts.insert(
            key.clone(),
            declared(declaration, mappings.get(key).copied()),
        );
    }
    for ((guid, key), observed) in observations {
        super::error::check(cancelled)?;
        let count = counts.entry(key.clone()).or_default();
        count.present += observed.present;
        count.populated += observed.populated;
        count.read_errors += observed.read_errors;
        count.missing_targets += observed.missing_targets;
        if projection.exclusions.contains_key(guid) {
            count.excluded += observed.present;
            continue;
        }
        let Some(mapping) = mappings.get(key) else {
            count.unhandled += observed.present;
            continue;
        };
        if mapping.previously_empty && observed.populated > 0 {
            count.population = Population::NewlyPopulated;
        }
        if projection.rejected.contains(guid) {
            count.rejected += observed.present;
        }
        if let Disposition::Exported { group, path } = &mapping.disposition {
            let rows = projection
                .groups
                .get(group)
                .and_then(|index| index.get(guid));
            let exported = count_exported(rows, path, cancelled)?;
            count.exported += exported;
            if rows.is_none() {
                count.missing_export_guid += observed.present;
            }
            count.unhandled += observed.populated.saturating_sub(exported);
        }
    }
    for key in mappings.keys() {
        counts.entry(key.clone()).or_default();
    }
    Ok(counts)
}

fn count_exported(
    rows: Option<&Vec<&Value>>,
    path: &super::path::Path,
    cancelled: &impl Fn() -> bool,
) -> super::Result<u64> {
    let mut count = 0_u64;
    let rows = rows.into_iter().flatten();
    for row in rows {
        count += path
            .select(row, cancelled)?
            .iter()
            .filter(|value| populated(value))
            .count() as u64;
    }
    Ok(count)
}

pub(crate) fn regressions(counts: &BTreeMap<FieldKey, Counts>) -> usize {
    counts
        .values()
        .filter(|count| {
            !count.declared
                || count.review == Review::Unreviewed
                || count.unhandled > 0
                || count.rejected > 0
                || count.read_errors > 0
                || count.missing_export_guid > 0
                || count.target == Target::Changed
                || count.population == Population::NewlyPopulated
        })
        .count()
}
