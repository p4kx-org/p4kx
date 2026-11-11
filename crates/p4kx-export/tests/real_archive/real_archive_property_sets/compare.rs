// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Record, RecordTree, TreeBuilder};
use serde_json::Value;

use super::error::{
    self, ArrayShape, Failure, JoinFault, OutputFault, ProductionFailure, ReadFault, SourceFault,
};
use super::health::Health;
use super::published::Published;
use super::{Counts, OracleResult, health, source, values};
use crate::real_archive_contracts::v3;

struct SourceOutcome {
    ambiguous: bool,
    failure: ProductionFailure,
}

pub(super) fn check<'f>(
    record: &Record<'f>,
    trees: &mut TreeBuilder<'f>,
    published: &Published<'_>,
    health: &Health<'_>,
    checked: &mut Counts,
) -> OracleResult {
    checked.sources += 1;
    let Ok(tree) = trees.build(record) else {
        if !health::read_failed(health, record.index()) {
            return Err(Failure::IndependentRead(
                ReadFault::UnreportedCanonicalFailure,
            ));
        }
        checked.failed += 1;
        return Ok(());
    };
    let owner = record.guid().to_string();
    let rows = published.contracts_of(record.index(), &owner);
    let sets: Vec<_> = published
        .sets
        .iter()
        .filter(|set| set["recordGuid"] == owner)
        .collect();
    check_owner(
        &tree,
        rows,
        &sets,
        &published.by_guid,
        &SourceOutcome {
            ambiguous: health::ambiguous(health, record.index()),
            failure: health::production_failure(health, record.index()),
        },
        checked,
    )
}

fn check_owner(
    tree: &RecordTree<'_>,
    rows: &[&Value],
    sets: &[&Value],
    by_guid: &HashMap<&str, &Value>,
    outcome: &SourceOutcome,
    checked: &mut Counts,
) -> OracleResult {
    let owner = tree.guid().to_string();
    let raw = source::read(tree.root())?;
    for (location, source) in &raw.sets {
        let Some(set) = sets
            .iter()
            .find(|set| set["sourceLocation"] == location.as_str())
        else {
            if !outcome.ambiguous {
                return Err(Failure::SourceCompare(SourceFault::MissingAuthoredArray {
                    level: source.level,
                    shape: if source.properties.is_empty() {
                        ArrayShape::Empty
                    } else {
                        ArrayShape::Populated
                    },
                    production: outcome.failure,
                }));
            }
            checked.quarantined += 1;
            continue;
        };
        checked.scoped += check_set(tree, source, set)?;
        checked.sets.insert(
            set["guid"]
                .as_str()
                .ok_or(Failure::OutputParse(OutputFault::SetIdentity))?
                .to_owned(),
        );
    }
    for row in rows {
        let (contract, sub) =
            v3::identity(row).map_err(|_| Failure::Joins(JoinFault::ContractIdentity))?;
        let key = (contract.to_owned(), sub.map(str::to_owned));
        let Some(expected) = raw.rows.get(&key) else {
            continue;
        };
        check_joins(row, expected, by_guid, &owner, outcome.ambiguous)?;
        checked.rows.insert(key);
    }
    Ok(())
}

fn check_set(
    tree: &RecordTree<'_>,
    source: &source::SourceSet<'_, '_>,
    set: &Value,
) -> OracleResult<usize> {
    require_equal!(
        set["recordPath"],
        tree.record_path(),
        Failure::SourceCompare(SourceFault::SetPath)
    );
    require_equal!(
        set["sourceType"],
        tree.type_name(),
        Failure::SourceCompare(SourceFault::SetType)
    );
    require_equal!(
        set["level"],
        source.level.name(),
        Failure::SourceCompare(SourceFault::SetLevel)
    );
    values::check(source.properties, &set["properties"], tree.record_index())
}

fn check_joins(
    row: &Value,
    expected: &[String],
    sets: &HashMap<&str, &Value>,
    owner: &str,
    ambiguous: bool,
) -> OracleResult {
    let expected: Vec<_> = expected
        .iter()
        .filter(|location| {
            !ambiguous
                || sets.values().any(|set| {
                    set["recordGuid"] == owner && set["sourceLocation"] == location.as_str()
                })
        })
        .collect();
    let actual = row["propertySetGuids"]
        .as_array()
        .ok_or(Failure::Joins(JoinFault::ArrayShape))?;
    require_equal!(
        actual.len(),
        expected.len(),
        Failure::Joins(JoinFault::Count)
    );
    let joins = actual.iter().zip(expected);
    for (guid, location) in joins {
        let set = sets
            .get(guid.as_str().ok_or(Failure::Joins(JoinFault::Target))?)
            .ok_or(Failure::Joins(JoinFault::Target))?;
        require_equal!(set["recordGuid"], owner, Failure::Joins(JoinFault::Owner));
        require_equal!(
            set["sourceLocation"],
            location.as_str(),
            Failure::Joins(JoinFault::Location)
        );
    }
    Ok(())
}
