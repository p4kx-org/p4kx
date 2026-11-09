// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_export::{Kind, is_kind_leftover};
use p4kx_extraction::{
    ArchiveWatch, DataForge, DataForgeOrigin, Guid, Record, RecordTree, TreeBuilder, Value as Raw,
};
use serde_json::Value;

use super::carriers::{Carriers, Uses};
use super::{Failure, Result, compare};

#[path = "group_values.rs"]
mod values;

#[derive(Clone, Copy)]
struct PhysicalGroup {
    guid: Guid,
    index: u32,
}

struct Parent {
    source: PhysicalGroup,
    ordinal: usize,
}

#[derive(Default)]
struct Expected {
    selected: Vec<PhysicalGroup>,
    parents: HashMap<Guid, Parent>,
}

/// Compares all selected physical group facts within the existing build-bound oracle budget.
pub(super) fn check(
    forge: &DataForge<'_>,
    origin: &DataForgeOrigin,
    catalog: &Value,
    watch: &ArchiveWatch,
) -> Result<usize> {
    let definitions = rows(catalog, "/definitions/resourceGroups/rows")?;
    let members = rows(catalog, "/declarations/resourceGroupMembers/rows")?;
    let parents = rows(catalog, "/projections/resourceGroupParents/rows")?;
    let uses = Uses::new(catalog, origin)?;
    let expected = source_groups(forge, definitions, members, catalog, &uses, watch)?;
    let count = expected.selected.len();
    require(
        definitions.len() == count && members.len() == count && parents.len() == count,
        "physical-group-cardinality",
    )?;
    parent_choices(parents, &expected, &uses)?;
    Ok(count)
}

fn source_groups<'catalog>(
    forge: &DataForge<'_>,
    definitions: &'catalog [Value],
    members: &'catalog [Value],
    catalog: &'catalog Value,
    uses: &Uses<'catalog>,
    watch: &ArchiveWatch,
) -> Result<Expected> {
    let mut builder = TreeBuilder::new();
    let mut expected = Expected::default();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|error| Failure::source("group-source-changed", error))?;
        if !selected(&record)? {
            continue;
        }
        let tree = builder
            .build(&record)
            .map_err(|error| Failure::source("group-tree", error))?;
        let definition = physical(definitions, record.index())?;
        let member = physical(members, record.index())?;
        values::definition(&tree, definition, catalog, uses)?;
        values::members(&tree, member, uses)?;
        expected.add(&tree);
    }
    Ok(expected)
}

fn selected(record: &Record<'_>) -> Result<bool> {
    let path = record
        .record_path()
        .map_err(|error| Failure::source("group-path", error))?;
    let rule = Kind::ResourceGroups
        .rule()
        .ok_or_else(|| Failure::check("group-selection-rule"))?;
    if !rule.matches_path(path) {
        return Ok(false);
    }
    let kind = record
        .type_name()
        .map_err(|error| Failure::source("group-type", error))?;
    if kind != rule.type_name {
        return Ok(false);
    }
    let name = record
        .name()
        .map_err(|error| Failure::source("group-name", error))?;
    Ok(!is_kind_leftover(Kind::ResourceGroups, path, name))
}

impl Expected {
    fn add(&mut self, tree: &RecordTree<'_>) {
        let source = PhysicalGroup {
            guid: tree.guid(),
            index: tree.record_index(),
        };
        if let Some(Raw::Array(children)) = tree.field("groups") {
            for (ordinal, child) in children.iter().enumerate() {
                if let Raw::Reference(target) = child
                    && target.location().is_none()
                    && target.guid() != tree.guid()
                {
                    self.parents
                        .entry(target.guid())
                        .or_insert(Parent { source, ordinal });
                }
            }
        }
        self.selected.push(source);
    }
}

fn parent_choices<'catalog>(
    rows: &'catalog [Value],
    expected: &Expected,
    uses: &Uses<'catalog>,
) -> Result<()> {
    for selected in &expected.selected {
        let row = physical(rows, selected.index)?;
        let carrier = row
            .get("parent")
            .ok_or_else(|| Failure::check("group-parent-carrier"))?;
        let parent =
            compare::present(carrier).ok_or_else(|| Failure::check("group-parent-value"))?;
        let carriers = Carriers::new(row, uses, selected.index)?;
        carriers.require_at(carrier, selected.index, "$guid")?;
        match expected.parents.get(&selected.guid) {
            None => require(parent.is_null(), "group-parent-none")?,
            Some(chosen) => {
                require(
                    parent.pointer("/guid/value").and_then(Value::as_str)
                        == Some(chosen.source.guid.to_string().as_str()),
                    "group-parent-guid",
                )?;
                require(
                    parent
                        .pointer("/targetRecordIndex/value")
                        .and_then(Value::as_u64)
                        == Some(u64::from(chosen.source.index)),
                    "group-parent-physical-index",
                )?;
                carriers.require_at(
                    carrier,
                    chosen.source.index,
                    &format!("groups[{}]", chosen.ordinal),
                )?;
            }
        }
    }
    Ok(())
}

fn rows<'a>(catalog: &'a Value, path: &str) -> Result<&'a [Value]> {
    catalog
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| Failure::check("canonical-group-rows"))
}

fn physical(rows: &[Value], index: u32) -> Result<&Value> {
    let mut matching = rows.iter().filter(|row| {
        row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(index))
    });
    let row = matching
        .next()
        .ok_or_else(|| Failure::check("physical-group-row"))?;
    require(matching.next().is_none(), "duplicate-physical-group-row")?;
    Ok(row)
}

const fn require(condition: bool, stage: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Failure::check(stage))
    }
}
