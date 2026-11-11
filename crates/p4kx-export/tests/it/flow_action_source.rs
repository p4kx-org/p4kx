// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, SourceEncryption, StructValue, Value};

use crate::flow_action_fixture::{self as fixture, Case, Names};
use crate::flow_action_flow_source;
use crate::flow_action_target_source;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    Field(usize),
    Element(usize),
}

pub(crate) struct Original {
    paths: Vec<Vec<Step>>,
}

impl Original {
    pub(crate) fn ordinal(&self, path: &[Step]) -> TestResult<u64> {
        let found = self
            .paths
            .iter()
            .position(|actual| actual == path)
            .ok_or("actual original occurrence path")?;
        Ok(u64::try_from(found)?)
    }
}

pub(crate) fn check(
    input: &Fixture,
    case: Case,
    names: Names,
    origin: Input,
) -> TestResult<Original> {
    input_origin(input, origin);
    let forge = input.entry.open()?;
    assert_eq!(
        forge.records().len(),
        2,
        "two genuine physical source Records, including a distinct header-owner control"
    );
    other_owner(&forge)?;
    let record = forge.record(0).ok_or("actual original template")?;
    assert_eq!(
        record.guid().to_string(),
        fixture::OWNER,
        "actual owner GUID"
    );
    assert_eq!(
        record.type_name()?,
        "ContractTemplate",
        "actual selected source family"
    );
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "paired original source tree");
    assert_eq!(
        tree.error_count(),
        if matches!(case, Case::Unreadable) {
            4
        } else {
            0
        },
        "only the four intentional target-field read failures are unreadable"
    );
    roots(tree.root(), case, names)?;
    flow_action_flow_source::check(
        structure(tree.field("missionFlow").ok_or("actual original flow")?)?,
        case,
        names,
        &record,
    )?;
    let original = Original {
        paths: enumerate(tree.root()),
    };
    assert_eq!(
        original.ordinal(&[Step::Field(0)])?,
        0,
        "first actual target precedes Flow in source DFS"
    );
    let flow = original.ordinal(&[Step::Field(1)])?;
    let later = original.ordinal(&[Step::Field(2)])?;
    assert!(
        flow > 0 && later > flow,
        "the other actual target follows the whole Flow subtree"
    );
    Ok(original)
}

fn roots(root: &StructValue<'_>, case: Case, names: Names) -> TestResult {
    assert_eq!(root.fields().len(), 3, "all original ordered declarations");
    for (index, expected) in [
        (0, "target"),
        (1, "missionFlow"),
        (
            2,
            if matches!(names, Names::Duplicate) {
                "target"
            } else {
                "targeT"
            },
        ),
    ] {
        let field = root.fields().get(index).ok_or("actual root declaration")?;
        assert_eq!(
            field.name(),
            expected,
            "actual root field ordinal/name, including duplicates"
        );
        assert_eq!(
            field.storage().data_type(),
            if index == 1 {
                DataType::Class
            } else {
                DataType::StrongPointer
            },
            "actual canonical storage"
        );
    }
    flow_action_target_source::check(
        structure(
            root.fields()
                .first()
                .ok_or("earlier actual target")?
                .value(),
        )?,
        case,
        false,
    )?;
    let earlier = root
        .fields()
        .first()
        .ok_or("actual earlier source")?
        .value();
    let later = root.fields().get(2).ok_or("actual later source")?.value();
    if matches!(case, Case::EqualValues) {
        assert_eq!(
            earlier, later,
            "equal actual source contents do not identify their physical occurrence"
        );
        assert!(
            !std::ptr::eq(earlier, later),
            "different physical source slots with equal values"
        );
    }
    flow_action_target_source::check(structure(later)?, case, true)
}

pub(crate) fn array<'tree, 'source>(
    root: &'tree StructValue<'source>,
    name: &str,
) -> TestResult<&'tree [Value<'source>]> {
    match root.field(name) {
        Some(Value::Array(values)) => Ok(values),
        _ => Err("actual original array".into()),
    }
}

pub(crate) fn structure<'tree, 'source>(
    value: &'tree Value<'source>,
) -> TestResult<&'tree StructValue<'source>> {
    match value {
        Value::Struct(value) => Ok(value),
        _ => Err("actual original Struct".into()),
    }
}

fn enumerate(root: &StructValue<'_>) -> Vec<Vec<Step>> {
    let mut pending: Vec<_> = root
        .fields()
        .iter()
        .enumerate()
        .rev()
        .map(|(index, field)| (field.value(), vec![Step::Field(index)]))
        .collect();
    let mut paths = Vec::new();
    while let Some((value, path)) = pending.pop() {
        paths.push(path.clone());
        match value {
            Value::Struct(value) => {
                for (index, field) in value.fields().iter().enumerate().rev() {
                    let mut next = path.clone();
                    next.push(Step::Field(index));
                    pending.push((field.value(), next));
                }
            }
            Value::Array(values) => {
                for (index, value) in values.iter().enumerate().rev() {
                    let mut next = path.clone();
                    next.push(Step::Element(index));
                    pending.push((value, next));
                }
            }
            _ => {}
        }
    }
    paths
}

fn input_origin(input: &Fixture, origin: Input) {
    let encryption = match origin {
        Input::Plain => SourceEncryption::Unencrypted,
        Input::Encrypted => SourceEncryption::Encrypted,
        Input::Standalone => SourceEncryption::Unknown,
    };
    assert_eq!(
        input.entry.origin().encryption(),
        encryption,
        "actual original input encryption before output"
    );
    assert_eq!(
        input.entry.origin().matches_archive(&input.archive),
        !matches!(origin, Input::Standalone),
        "actual original input owner"
    );
}

fn other_owner(forge: &DataForge<'_>) -> TestResult {
    let other = forge.record(1).ok_or("genuine other physical owner")?;
    assert_eq!(
        other.guid().to_string(),
        "f1700000-0000-4000-8000-000000000002",
        "real distinct emitted header owner"
    );
    assert_eq!(
        other.tree()?.error_count(),
        0,
        "genuine other owner is independently readable"
    );
    Ok(())
}
