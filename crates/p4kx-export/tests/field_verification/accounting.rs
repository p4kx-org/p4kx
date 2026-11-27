// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::Guid;
use serde_json::Value;

use super::inventory::{BuildIdentity, Disposition, FieldKey, Inventory};
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeclarationState {
    #[default]
    Missing,
    Declared,
    ExpectedAbsent,
    UnexpectedlyDeclared,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Counts {
    pub(crate) declaration: DeclarationState,
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

impl Counts {
    fn review_expected_absence(&mut self) {
        self.declaration = if self.declaration == DeclarationState::Missing && self.present == 0 {
            DeclarationState::ExpectedAbsent
        } else {
            DeclarationState::UnexpectedlyDeclared
        };
        self.review = Review::Reviewed;
    }
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
        declaration: DeclarationState::Declared,
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

pub(crate) fn count_fields(
    schema: &Schema,
    observations: &Observations,
    inventory: &Inventory,
    projection: &Projection<'_>,
    build: Option<&BuildIdentity>,
    cancelled: &impl Fn() -> bool,
) -> super::Result<BTreeMap<FieldKey, Counts>> {
    let mappings = inventory.reviewed()?;
    let expected_absences = inventory.absent_on(build);
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
            let compared = count_exported(rows, path, cancelled)?;
            count.exported += compared.exported;
            if rows.is_none() {
                count.missing_export_guid += observed.present;
            }
            count.unhandled += if path.counts_original_occurrences() {
                observed
                    .present
                    .abs_diff(compared.exported)
                    .max(observed.populated.abs_diff(compared.populated))
                    .max(observed.null.abs_diff(compared.null))
            } else {
                observed.populated.saturating_sub(compared.exported)
            };
        }
    }
    for key in mappings.keys() {
        let count = counts.entry(key.clone()).or_default();
        if expected_absences.contains(key) && schema.owners.contains_key(&key.owner) {
            count.review_expected_absence();
        }
    }
    Ok(counts)
}

#[derive(Default)]
struct Compared {
    exported: u64,
    populated: u64,
    null: u64,
}

fn count_exported(
    rows: Option<&Vec<&Value>>,
    path: &super::path::Path,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Compared> {
    let mut counts = Compared::default();
    let original_occurrences = path.counts_original_occurrences();
    for row in rows.into_iter().flatten() {
        for value in path.select(row, cancelled)? {
            if original_occurrences {
                counts.exported += 1;
                counts.null +=
                    u64::from(value.get("state").and_then(Value::as_str) == Some("null"));
                counts.populated += u64::from(super::arena_path::member_populated(value)?);
            } else {
                counts.exported += u64::from(populated(value));
            }
        }
    }
    Ok(counts)
}

pub(crate) fn regressions(counts: &BTreeMap<FieldKey, Counts>) -> usize {
    counts
        .values()
        .filter(|count| {
            matches!(
                count.declaration,
                DeclarationState::Missing | DeclarationState::UnexpectedlyDeclared
            ) || count.review == Review::Unreviewed
                || count.unhandled > 0
                || count.rejected > 0
                || count.read_errors > 0
                || count.missing_export_guid > 0
                || count.target == Target::Changed
                || count.population == Population::NewlyPopulated
        })
        .count()
}
