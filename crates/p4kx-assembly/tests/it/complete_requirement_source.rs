// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{RecordTree, StructValue, Value};

use crate::complete_requirement_fixture::{Site, TestResult};

pub(crate) fn requirement<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    site: Site,
) -> TestResult<Option<&'tree Value<'source>>> {
    let Some(Value::Array(groups)) = tree.field("GroupedLoadouts") else {
        return Err("original grouped loadouts array".into());
    };
    assert_eq!(groups.len(), 2, "all original groups before selection");
    let names: Vec<_> = groups
        .iter()
        .map(|value| {
            let group = structure(value)?;
            let Some(Value::String(name)) = group.field("Group") else {
                return Err("original group name".into());
            };
            Ok(*name)
        })
        .collect::<TestResult<_>>()?;
    assert_eq!(names, ["Body", "Legs"], "original authored group order");
    match site {
        Site::Assortment => Ok(tree.field("Requirement")),
        Site::Group => {
            Ok(structure(groups.first().ok_or("original Body group")?)?.field("Requirement"))
        }
    }
}

pub(crate) fn structure<'value, 'source>(
    value: &'value Value<'source>,
) -> TestResult<&'value StructValue<'source>> {
    match value {
        Value::Struct(structure) => Ok(structure),
        _ => Err("actual original Requirement struct".into()),
    }
}

pub(crate) fn chain(value: &Value<'_>, depth: usize, tagged: bool) -> TestResult {
    let mut current = value;
    for _ in 0..depth {
        let part = structure(current)?;
        assert_eq!(
            part.type_name(),
            "SLoadoutRequirementNOT",
            "every original NOT in authored order"
        );
        current = part
            .field("RequirementToInvert")
            .ok_or("original NOT operand")?;
    }
    tags(current, tagged)
}

pub(crate) fn tags(value: &Value<'_>, tagged: bool) -> TestResult {
    let terminal = structure(value)?;
    assert_eq!(
        terminal.type_name(),
        "SLoadoutRequirementTags",
        "exact terminal after every original NOT"
    );
    let Some(Value::Array(required)) = terminal.field("RequiredTags") else {
        return Err("original required tags array".into());
    };
    let Some(Value::Array(banned)) = terminal.field("BannedTags") else {
        return Err("original banned tags array".into());
    };
    assert!(banned.is_empty(), "actual empty banned tags");
    if tagged {
        assert_eq!(required.len(), 1, "one genuine authored Tag reference");
        let Some(Value::Reference(target)) = required.first() else {
            return Err("actual resolved Tag reference".into());
        };
        assert_eq!(target.type_name(), "Tag", "genuine declared Tag target");
        assert_eq!(
            target.record_path(),
            "tags/required",
            "actual original Tag target path"
        );
    } else {
        assert!(required.is_empty(), "actual unrestricted terminal");
    }
    Ok(())
}

pub(crate) fn parts<'value, 'source>(
    value: &'value Value<'source>,
) -> TestResult<&'value [Value<'source>]> {
    let value = structure(value)?;
    assert_eq!(
        value.type_name(),
        "SLoadoutRequirementOR",
        "actual original OR type"
    );
    match value.field("Requirements") {
        Some(Value::Array(parts)) => Ok(parts),
        _ => Err("original OR operands array".into()),
    }
}
