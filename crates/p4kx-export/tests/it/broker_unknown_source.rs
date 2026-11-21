// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Field, RecordTree, StructValue, Value};

use crate::broker_unknown_fixture::{Case, GUID};
use crate::health_origin_fixture::{Fixture, TestResult};
use crate::mission_property_remaining_source::Original;

fn with_tree<T>(
    input: &Fixture,
    check: impl FnOnce(&RecordTree<'_>) -> TestResult<T>,
) -> TestResult<T> {
    let forge = input.entry.open()?;
    let record = forge.record(0).ok_or("actual physical Broker Record")?;
    assert!(
        forge.record(1).is_none(),
        "exactly one physical original owner"
    );
    assert_eq!(
        record.guid().to_string(),
        crate::health_origin_fixture::guid(GUID),
        "genuine original GUID"
    );
    assert_eq!(
        record.type_name()?,
        "MissionBrokerEntry",
        "genuine selected Broker family"
    );
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the tree is from this exact original Record"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "the unsupported meaning has fully readable original facts"
    );
    let names: Vec<_> = tree.fields().iter().map(Field::name).collect();
    assert_eq!(
        names,
        ["reputationRequirements", "modifiers"],
        "all exact original root declarations"
    );
    let first = tree
        .fields()
        .first()
        .ok_or("actual original requirements declaration")?;
    let second = tree
        .fields()
        .get(1)
        .ok_or("actual original modifiers declaration")?;
    for (field, array) in [(first, false), (second, true)] {
        assert_eq!(
            field.storage().data_type(),
            DataType::StrongPointer,
            "actual declared pointer storage"
        );
        assert_eq!(
            field.storage().is_array(),
            array,
            "actual scalar versus array declaration"
        );
    }
    check(&tree)
}

pub(crate) fn check(input: &Fixture, case: Case) -> TestResult<Original> {
    with_tree(input, |tree| {
        let values = values(tree);
        assert_eq!(
            values.len(),
            case.total(),
            "the complete original DFS inventory precedes projection"
        );
        let matching: Vec<_> = values
            .iter()
            .enumerate()
            .filter_map(|(ordinal, value)| match value {
                Value::Struct(source) if source.type_name() == case.source_type() => {
                    Some((ordinal, source))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            matching.len(),
            1,
            "one genuinely selected readable unsupported occurrence"
        );
        let (ordinal, source) = matching
            .first()
            .ok_or("actual unsupported original Struct")?;
        let ordinal = u64::try_from(*ordinal)?;
        assert_eq!(
            ordinal,
            case.ordinal(),
            "the independently visited original ordinal"
        );
        assert_unknown_payload(source)?;
        preceding_known(tree, case)?;
        Ok(Original {
            source_type_ordinal: ordinal,
            field_uses: Vec::new(),
        })
    })
}

fn assert_unknown_payload(source: &StructValue<'_>) -> TestResult {
    let declaration = source
        .fields()
        .first()
        .ok_or("actual untouched extra source fact")?;
    assert_eq!(
        source.fields().len(),
        1,
        "the original unsupported subtree has its own field"
    );
    assert_eq!(
        declaration.name(),
        "originalPayload",
        "exact original authored declaration"
    );
    assert_eq!(
        declaration.storage().data_type(),
        DataType::UInt64,
        "exact original stored width"
    );
    assert!(
        !declaration.storage().is_array(),
        "actual scalar declaration"
    );
    assert_eq!(
        declaration.value(),
        &Value::UInt64(u64::MAX),
        "exact readable untouched original maximum"
    );
    Ok(())
}

fn preceding_known(tree: &RecordTree<'_>, case: Case) -> TestResult {
    let values = values(tree);
    let known = match case {
        Case::Requirements => return Ok(()),
        Case::Term => "SReputationMissionRequirementExpression_And",
        Case::Modifier => "MissionModifier_LawLicense",
    };
    let Some(Value::Struct(source)) = values.get(2).copied() else {
        return Err("actual earlier known Struct".into());
    };
    assert_eq!(
        source.type_name(),
        known,
        "the earlier known original is not the failing occurrence"
    );
    Ok(())
}

pub(crate) fn positive(input: &Fixture) -> TestResult {
    with_tree(input, |tree| {
        let values = values(tree);
        assert_eq!(values.len(), 5, "all known original occurrences");
        let types: Vec<_> = values
            .iter()
            .filter_map(|value| match value {
                Value::Struct(source) => Some(source.type_name()),
                _ => None,
            })
            .collect();
        assert_eq!(
            types,
            [
                "SReputationMissionRequirementsParams",
                "SReputationMissionRequirementExpression_And",
                "MissionModifier_LawLicense"
            ],
            "all original meanings are recognized before publication"
        );
        Ok(())
    })
}

fn values<'tree, 'source>(tree: &'tree RecordTree<'source>) -> Vec<&'tree Value<'source>> {
    let mut pending: Vec<_> = tree.fields().iter().rev().map(Field::value).collect();
    let mut values = Vec::new();
    while let Some(value) = pending.pop() {
        values.push(value);
        match value {
            Value::Struct(source) => pending.extend(source.fields().iter().rev().map(Field::value)),
            Value::Array(items) => pending.extend(items.iter().rev()),
            _ => {}
        }
    }
    values
}
