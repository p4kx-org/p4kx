// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::slice;

use p4kx_extraction::{Field, Guid, StructValue, Value};

use super::TestResult;
use crate::real_archive_contract_templates::mission_graph::Family;

pub(super) const AUTO_SPAWNS: &str = "MissionPropertyValue_AutoSpawns";

pub(super) struct Selected<'tree, 'source> {
    pub(super) property: &'tree Value<'source>,
    pub(super) objective: Option<&'tree Value<'source>>,
    pub(super) value: &'tree Value<'source>,
}

pub(super) fn selected<'tree, 'source>(
    root: &'tree StructValue<'source>,
    family: Family,
) -> Vec<Selected<'tree, 'source>> {
    let name = match family {
        Family::Template => "contractProperties",
        Family::Broker => "properties",
    };
    let mut found = Vec::new();
    for property in array(root.field(name)) {
        retain(&mut found, property, None, structure(property));
    }
    let mut objectives = Vec::new();
    objectives.extend(array(root.field("objectiveTokens")).iter().rev());
    while let Some(owner) = objectives.pop() {
        let Some(objective) = structure(owner) else {
            continue;
        };
        if !matches!(objective.field("id"), Some(Value::Guid(id)) if *id != Guid::default()) {
            continue;
        }
        if let Some(properties) = objective.field("properties") {
            for property in owned_properties(properties) {
                let embedded =
                    structure(property).and_then(|source| first_struct(source.field("property")));
                retain(&mut found, property, Some(owner), embedded);
            }
        }
        objectives.extend(array(objective.field("childMissionPhases")).iter().rev());
    }
    found
}

fn retain<'tree, 'source>(
    found: &mut Vec<Selected<'tree, 'source>>,
    property: &'tree Value<'source>,
    objective: Option<&'tree Value<'source>>,
    naming: Option<&'tree StructValue<'source>>,
) {
    let value = naming
        .and_then(|source| first_struct_value(source.field("value")))
        .filter(|value| structure(value).is_some_and(|value| value.type_name() == AUTO_SPAWNS));
    if let Some(value) = value {
        found.push(Selected {
            property,
            objective,
            value,
        });
    }
}

pub(super) fn resolved_reference(case: &Selected<'_, '_>) -> bool {
    matches!(
        structure(case.value).and_then(|source| source.field("autoSpawns")),
        Some(Value::Reference(link)) if link.location().is_none()
    )
}

pub(super) fn populated_closet(case: &Selected<'_, '_>) -> bool {
    let Some(value) = structure(case.value) else {
        return false;
    };
    resolved_reference(case)
        && value
            .field("searchOptions")
            .and_then(structure)
            .is_some_and(|options| {
                options.type_name() == "SpawnClosetSearchOptions_Tags"
                    && ["closetPositiveTags", "closetNegativeTags"]
                        .iter()
                        .any(|field| options.field(field).and_then(structure).is_some())
            })
}

fn owned_properties<'tree, 'source>(value: &'tree Value<'source>) -> &'tree [Value<'source>] {
    match value {
        Value::Struct(_) => slice::from_ref(value),
        Value::Array(items) => items,
        _ => &[],
    }
}

fn array<'tree, 'source>(value: Option<&'tree Value<'source>>) -> &'tree [Value<'source>] {
    match value {
        Some(Value::Array(items)) => items,
        _ => &[],
    }
}

fn first_struct<'tree, 'source>(
    value: Option<&'tree Value<'source>>,
) -> Option<&'tree StructValue<'source>> {
    first_struct_value(value).and_then(structure)
}

fn first_struct_value<'tree, 'source>(
    value: Option<&'tree Value<'source>>,
) -> Option<&'tree Value<'source>> {
    match value? {
        direct @ Value::Struct(_) => Some(direct),
        Value::Array(items) => items.iter().find(|value| structure(value).is_some()),
        _ => None,
    }
}

pub(super) const fn structure<'tree, 'source>(
    value: &'tree Value<'source>,
) -> Option<&'tree StructValue<'source>> {
    match value {
        Value::Struct(source) => Some(source),
        _ => None,
    }
}

pub(crate) struct Locations<'source>(HashMap<*const Value<'source>, usize>);

enum Children<'tree, 'source> {
    Fields(slice::Iter<'tree, Field<'source>>),
    Elements(slice::Iter<'tree, Value<'source>>),
}

impl<'tree, 'source> Children<'tree, 'source> {
    fn next(&mut self) -> Option<&'tree Value<'source>> {
        match self {
            Self::Fields(fields) => fields.next().map(Field::value),
            Self::Elements(elements) => elements.next(),
        }
    }
}

impl<'source> Locations<'source> {
    pub(crate) fn read(root: &StructValue<'source>) -> Self {
        let mut stack = vec![Children::Fields(root.fields().iter())];
        let mut ordinals = HashMap::new();
        while let Some(children) = stack.last_mut() {
            let Some(value) = children.next() else {
                stack.pop();
                continue;
            };
            ordinals.insert(std::ptr::from_ref(value), ordinals.len());
            match value {
                Value::Struct(source) => stack.push(Children::Fields(source.fields().iter())),
                Value::Array(items) => stack.push(Children::Elements(items.iter())),
                _ => {}
            }
        }
        Self(ordinals)
    }

    pub(crate) fn ordinal(&self, value: &Value<'source>) -> TestResult<usize> {
        self.0
            .get(&std::ptr::from_ref(value))
            .copied()
            .ok_or_else(|| "actual source occurrence ordinal".into())
    }
}
