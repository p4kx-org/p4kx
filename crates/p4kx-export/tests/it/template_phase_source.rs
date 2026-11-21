// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, StructValue, Value};

use crate::commodity_v3_fixture;
use crate::health_origin_fixture::{Fixture, TestResult};
use crate::template_phase_fixture as fixture;

pub(crate) struct Observed {
    pub(crate) objectives: Vec<Objective>,
}

pub(crate) struct Objective {
    pub(crate) id: String,
    pub(crate) parent: Option<String>,
    pub(crate) name: String,
}

pub(crate) fn check(input: &Fixture, requested: usize) -> TestResult<Observed> {
    let forge = input.entry.open()?;
    let definition = forge
        .structs()
        .find(|definition| definition.name().is_ok_and(|name| name == "ObjectiveToken"))
        .ok_or("original ObjectiveToken declaration")?;
    assert_eq!(
        definition.instance_count(),
        u32::try_from(requested.checked_add(1).ok_or("synthetic phase count")?)?,
        "all original phase instances and trailing sibling exist before the canonical read"
    );
    let record = forge.record(0).ok_or("original template Record")?;
    assert_eq!(
        record.guid().to_string(),
        fixture::TEMPLATE_TEXT,
        "original GUID"
    );
    assert_eq!(
        record.type_name()?,
        "ContractTemplate",
        "actual source family"
    );
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the actual tree belongs to its Record"
    );
    assert_eq!(tree.fields().len(), 1, "one original root declaration");
    let root = tree.fields().first().ok_or("original root field")?;
    assert_eq!(root.name(), "objectiveTokens", "actual root name");
    assert_eq!(
        root.storage().data_type(),
        DataType::Class,
        "actual Class array"
    );
    assert!(root.storage().is_array(), "original root is an array");
    let roots = array(tree.root(), "objectiveTokens")?;
    assert_eq!(roots.len(), 2, "one chain and one later source sibling");
    let mut objectives = chain(roots.first().ok_or("original chain")?, requested)?;
    assert_eq!(tree.error_count(), 0, "no hidden source error");
    objectives.push(trailing(roots.get(1).ok_or("actual later sibling")?)?);
    Ok(Observed { objectives })
}

fn chain(mut next: &Value<'_>, requested: usize) -> TestResult<Vec<Objective>> {
    let mut objectives = Vec::new();
    let mut parent = None;
    for ordinal in 0..requested {
        let node = structure(next)?;
        declaration(node);
        let id = commodity_v3_fixture::guid(fixture::id(ordinal)?);
        assert_eq!(
            node.field("id"),
            Some(&Value::Guid(id.parse()?)),
            "original phase ID"
        );
        let name = format!("phase-{ordinal}");
        assert_eq!(
            node.field("debugName"),
            Some(&Value::String(&name)),
            "original name"
        );
        objectives.push(Objective {
            id: id.clone(),
            parent,
            name,
        });
        parent = Some(id);
        let children = array(node, "childMissionPhases")?;
        if ordinal.checked_add(1).ok_or("source ordinal")? == requested {
            assert!(children.is_empty(), "the last actual phase has no children");
        } else {
            assert_eq!(children.len(), 1, "each original chain edge has one child");
            next = children.first().ok_or("actual child")?;
        }
    }
    Ok(objectives)
}

fn trailing(value: &Value<'_>) -> TestResult<Objective> {
    let node = structure(value)?;
    declaration(node);
    let id = commodity_v3_fixture::guid(fixture::TRAILING);
    assert_eq!(
        node.field("id"),
        Some(&Value::Guid(id.parse()?)),
        "actual later root ID"
    );
    assert_eq!(
        node.field("debugName"),
        Some(&Value::String("trailing")),
        "trailing name"
    );
    assert!(
        array(node, "childMissionPhases")?.is_empty(),
        "trailing leaf"
    );
    Ok(Objective {
        id,
        parent: None,
        name: "trailing".to_owned(),
    })
}

fn declaration(node: &StructValue<'_>) {
    assert_eq!(
        node.type_name(),
        "ObjectiveToken",
        "actual owned source type"
    );
    assert_eq!(node.fields().len(), 3, "all original declarations");
    for (field, (name, kind, array)) in node.fields().iter().zip([
        ("id", DataType::Guid, false),
        ("debugName", DataType::String, false),
        ("childMissionPhases", DataType::Class, true),
    ]) {
        assert_eq!(field.name(), name, "original declaration order");
        assert_eq!(field.storage().data_type(), kind, "original storage");
        assert_eq!(field.storage().is_array(), array, "original shape");
    }
}

fn array<'tree, 'source>(
    node: &'tree StructValue<'source>,
    name: &str,
) -> TestResult<&'tree [Value<'source>]> {
    match node.field(name) {
        Some(Value::Array(values)) => Ok(values),
        _ => Err("actual original array".into()),
    }
}

fn structure<'tree, 'source>(
    value: &'tree Value<'source>,
) -> TestResult<&'tree StructValue<'source>> {
    match value {
        Value::Struct(node) => Ok(node),
        _ => Err("actual original owned phase".into()),
    }
}
