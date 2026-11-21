// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Field, RecordTree, StructValue, Value, ValueError};
use serde_json::{Value as Json, json};

use crate::health_origin_fixture::{Fixture, TestResult};

use super::fixture::{Case, GENERATOR};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Site {
    pub(super) record: u32,
    pub(super) ordinal: u64,
}

#[derive(Clone)]
pub(super) struct Pick {
    pub(super) field: &'static str,
    pub(super) value: Json,
    pub(super) layer: &'static str,
    pub(super) datum: Site,
    pub(super) parameter: Option<Site>,
    pub(super) role: &'static str,
}

pub(super) struct Original {
    pub(super) parent: Vec<Pick>,
    pub(super) child: Vec<Pick>,
    pub(super) later: Option<Site>,
    pub(super) failed: Option<Site>,
    pub(super) shadowed_default: Option<Site>,
}

pub(super) fn read(fixture: &Fixture, case: Case) -> TestResult<Original> {
    let forge = fixture.entry.open()?;
    let record = forge.record(0).ok_or("original Generator")?;
    assert_eq!(
        record.guid().to_string(),
        crate::health_origin_fixture::guid(GENERATOR),
        "the original Generator identity"
    );
    assert_eq!(
        record.type_name()?,
        "ContractGenerator",
        "the original owner family"
    );
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the canonical tree has its exact original owner"
    );
    let handler = structure(first(tree.root(), "generators")?)?;
    let parent = structure(first(handler, "contracts")?)?;
    let availability = field(handler, "defaultAvailability")?;
    let defaults = structure(availability)?;
    assert_eq!(
        tree.error_count(),
        usize::from(matches!(case, Case::Unreadable)),
        "only the controlled original Bool byte can fail"
    );
    if matches!(case, Case::FullyOverridden) {
        return overridden_choices(&tree, parent, defaults, site(&tree, availability)?);
    }
    if matches!(case, Case::Defaults) {
        return default_choices(&tree, defaults);
    }
    if !matches!(case, Case::Layered) {
        let failed = if matches!(case, Case::Unreadable) {
            let value = field(defaults, "hasPersonalCooldown")?;
            assert_eq!(
                value,
                &Value::Error(ValueError::InvalidBool { byte: 3 }),
                "the original Bool byte is unreadable"
            );
            Some(site(&tree, value)?)
        } else {
            assert!(
                defaults.field("hasPersonalCooldown").is_none(),
                "the source declaration has no cooldown Bool"
            );
            None
        };
        return Ok(Original {
            parent: Vec::new(),
            child: Vec::new(),
            later: None,
            failed,
            shadowed_default: None,
        });
    }
    choices(&tree, handler, parent)
}

fn overridden_choices<'source>(
    tree: &RecordTree<'source>,
    parent: &StructValue<'source>,
    defaults: &StructValue<'source>,
    shadowed_default: Site,
) -> TestResult<Original> {
    assert_eq!(
        field(defaults, "personalCooldownTime")?,
        &Value::Single(120.0),
        "the shadowed original default is a readable alternative"
    );
    let params = structure(field(parent, "paramOverrides")?)?;
    let parent = [
        ("boolParamOverrides", 0, "hasPersonalCooldown", json!(false)),
        ("intParamOverrides", 0, "personalCooldownTime", json!(45.0)),
        (
            "intParamOverrides",
            1,
            "personalCooldownTimeVariation",
            json!(0.0),
        ),
        ("intParamOverrides", 2, "abandonedCooldownTime", json!(15.0)),
        (
            "intParamOverrides",
            3,
            "abandonedCooldownTimeVariation",
            json!(1.0),
        ),
    ]
    .into_iter()
    .map(|(array, index, field, value)| pick(tree, params, array, index, field, value, "contract"))
    .collect::<TestResult<Vec<_>>>()?;
    Ok(Original {
        parent,
        child: Vec::new(),
        later: None,
        failed: None,
        shadowed_default: Some(shadowed_default),
    })
}

fn default_choices<'source>(
    tree: &RecordTree<'source>,
    defaults: &StructValue<'source>,
) -> TestResult<Original> {
    let mut parent = Vec::new();
    for (name, expected) in [
        ("hasPersonalCooldown", json!(true)),
        ("personalCooldownTime", json!(120.0)),
        ("personalCooldownTimeVariation", json!(5.0)),
        ("abandonedCooldownTime", json!(30.0)),
        ("abandonedCooldownTimeVariation", json!(9.0)),
    ] {
        let value = field(defaults, name)?;
        let primitive = match value {
            Value::Bool(value) => json!(*value),
            Value::Single(value) => json!(f64::from(*value)),
            _ => return Err("the original default has no expected primitive".into()),
        };
        assert_eq!(primitive, expected, "the original stored default {name}");
        parent.push(Pick {
            field: name,
            value: expected,
            layer: "handler",
            datum: site(tree, value)?,
            parameter: None,
            role: "payload",
        });
    }
    Ok(Original {
        parent,
        child: Vec::new(),
        later: None,
        failed: None,
        shadowed_default: None,
    })
}

fn choices<'source>(
    tree: &RecordTree<'source>,
    handler: &StructValue<'source>,
    parent: &StructValue<'source>,
) -> TestResult<Original> {
    let handler_params = structure(field(handler, "contractParams")?)?;
    let contract_params = structure(field(parent, "paramOverrides")?)?;
    let child = structure(first(parent, "subContracts")?)?;
    let shared = shared_picks(tree, handler_params, contract_params)?;
    let mut parent_picks = vec![
        pick(
            tree,
            contract_params,
            "intParamOverrides",
            0,
            "personalCooldownTime",
            45.0.into(),
            "contract",
        )?,
        pick(
            tree,
            handler_params,
            "intParamOverrides",
            1,
            "abandonedCooldownTime",
            15.0.into(),
            "handler",
        )?,
    ];
    parent_picks.extend(shared.iter().cloned());
    let mut child_picks = vec![
        pick(
            tree,
            child,
            "intParamOverrides",
            0,
            "personalCooldownTime",
            7.0.into(),
            "subContract",
        )?,
        pick(
            tree,
            child,
            "intParamOverrides",
            1,
            "abandonedCooldownTime",
            0.0.into(),
            "subContract",
        )?,
    ];
    child_picks.extend(shared);
    Ok(Original {
        parent: parent_picks,
        child: child_picks,
        later: Some(parameter(tree, contract_params, "intParamOverrides", 1)?.datum),
        failed: None,
        shadowed_default: None,
    })
}

fn shared_picks<'source>(
    tree: &RecordTree<'source>,
    handler_params: &StructValue<'source>,
    contract_params: &StructValue<'source>,
) -> TestResult<Vec<Pick>> {
    Ok(vec![
        pick(
            tree,
            contract_params,
            "boolParamOverrides",
            0,
            "hasPersonalCooldown",
            false.into(),
            "contract",
        )?,
        pick(
            tree,
            handler_params,
            "intParamOverrides",
            0,
            "personalCooldownTimeVariation",
            0.0.into(),
            "handler",
        )?,
        pick(
            tree,
            contract_params,
            "intParamOverrides",
            2,
            "abandonedCooldownTimeVariation",
            0.0.into(),
            "contract",
        )?,
    ])
}

fn pick<'source>(
    tree: &RecordTree<'source>,
    owner: &StructValue<'source>,
    array: &str,
    index: usize,
    field_name: &'static str,
    expected: Json,
    layer: &'static str,
) -> TestResult<Pick> {
    let parameter = parameter(tree, owner, array, index)?;
    assert_eq!(
        parameter.stored, expected,
        "the exact original override value"
    );
    assert!(
        parameter.name.eq_ignore_ascii_case(field_name),
        "the actual authored parameter selects the expected field"
    );
    Ok(Pick {
        field: field_name,
        value: expected,
        layer,
        datum: parameter.datum,
        parameter: Some(parameter.selector),
        role: parameter.role,
    })
}

struct Parameter<'source> {
    datum: Site,
    selector: Site,
    name: &'source str,
    stored: Json,
    role: &'static str,
}

fn parameter<'source>(
    tree: &RecordTree<'source>,
    owner: &StructValue<'source>,
    array: &str,
    index: usize,
) -> TestResult<Parameter<'source>> {
    let Value::Array(values) = field(owner, array)? else {
        return Err("original parameter array".into());
    };
    let parameter = structure(values.get(index).ok_or("original stored parameter slot")?)?;
    let value = field(parameter, "value")?;
    let selector = field(parameter, "param")?;
    let &Value::String(name) = selector else {
        return Err("the original override has no String selector".into());
    };
    let (stored, role) = match value {
        Value::Bool(value) => (json!(*value), "payload"),
        Value::Int32(value) => (json!(f64::from(*value)), "conversion"),
        _ => return Err("the original override has no expected primitive".into()),
    };
    Ok(Parameter {
        datum: site(tree, value)?,
        selector: site(tree, selector)?,
        name,
        stored,
        role,
    })
}

fn first<'a, 's>(owner: &'a StructValue<'s>, name: &str) -> TestResult<&'a Value<'s>> {
    let Value::Array(values) = field(owner, name)? else {
        return Err("original owned array".into());
    };
    values
        .first()
        .ok_or_else(|| "original first owned occurrence".into())
}

fn structure<'a, 's>(value: &'a Value<'s>) -> TestResult<&'a StructValue<'s>> {
    let Value::Struct(value) = value else {
        return Err("original owned Struct".into());
    };
    Ok(value)
}

fn field<'a, 's>(owner: &'a StructValue<'s>, name: &str) -> TestResult<&'a Value<'s>> {
    owner
        .field(name)
        .ok_or_else(|| format!("original field {name}").into())
}

fn site<'source>(tree: &RecordTree<'source>, selected: &Value<'source>) -> TestResult<Site> {
    let mut pending: Vec<_> = tree.fields().iter().rev().map(Field::value).collect();
    let mut ordinal = 0_u64;
    while let Some(value) = pending.pop() {
        if std::ptr::eq(value, selected) {
            return Ok(Site {
                record: tree.record_index(),
                ordinal,
            });
        }
        ordinal = ordinal.checked_add(1).ok_or("original ordinal overflow")?;
        match value {
            Value::Struct(owner) => pending.extend(owner.fields().iter().rev().map(Field::value)),
            Value::Array(values) => pending.extend(values.iter().rev()),
            _ => {}
        }
    }
    Err("selected value has no original occurrence".into())
}
