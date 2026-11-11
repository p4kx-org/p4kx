// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Field, RecordTree, StructValue, Value};
use serde_json::{Value as Json, json};

use crate::health_origin_fixture::{self, Fixture, TestResult};

use super::super::{BARE, GENERATOR, OVERRIDDEN, SUB, TEMPLATE};

#[derive(Clone, Copy, Debug)]
pub(super) struct Site {
    pub(super) record: u32,
    pub(super) ordinal: u64,
}

pub(super) struct Fact {
    pub(super) field: &'static str,
    pub(super) layer: &'static str,
    pub(super) value: Json,
    pub(super) payload: Site,
    pub(super) selector: Site,
}

pub(super) struct Sources {
    pub(super) template: [Fact; 2],
    pub(super) defaults: [Fact; 5],
    pub(super) handler_hide: Fact,
    pub(super) shadowed_hide: Site,
    pub(super) parent: [Fact; 3],
    pub(super) sub_share: Fact,
    pub(super) bare_template: Site,
    pub(super) parent_template: Site,
}

impl Sources {
    pub(super) fn read(fixture: &Fixture) -> TestResult<Self> {
        let forge = fixture.entry.open()?;
        let generator = forge.record(3).ok_or("original Generator Record")?;
        let template = forge.record(0).ok_or("original Template Record")?;
        assert_eq!(
            generator.guid().to_string(),
            health_origin_fixture::guid(GENERATOR),
            "the layered Generator is the actual fixture owner"
        );
        assert_eq!(
            template.guid().to_string(),
            health_origin_fixture::guid(TEMPLATE),
            "the fallback Template is the actual fixture owner"
        );
        let generator_tree = generator.tree()?;
        let template_tree = template.tree()?;
        assert!(
            generator_tree.is_from(&generator) && template_tree.is_from(&template),
            "each value is found under its own physical Record"
        );
        assert_eq!(
            (generator_tree.error_count(), template_tree.error_count()),
            (0, 0),
            "the physical candidates are readable"
        );
        read_sources(&generator_tree, &template_tree)
    }
}

fn read_sources(generator: &RecordTree<'_>, template: &RecordTree<'_>) -> TestResult<Sources> {
    let handler = element(generator.root(), "generators", 0)?;
    let bare = element(handler, "contracts", 0)?;
    let parent = element(handler, "contracts", 1)?;
    let child = element(parent, "subContracts", 0)?;
    for (contract, id) in [(bare, BARE), (parent, OVERRIDDEN), (child, SUB)] {
        assert!(
            matches!(contract.field("id"), Some(Value::Guid(actual))
                if actual.to_string() == health_origin_fixture::guid(id)),
            "the selected physical occurrence stores its expected logical id"
        );
    }
    let availability = structure(field(handler, "defaultAvailability")?)?;
    let handler_params = structure(field(handler, "contractParams")?)?;
    let parent_params = structure(field(parent, "paramOverrides")?)?;
    assert!(
        child.field("paramOverrides").is_none(),
        "the sub contract states its own parameters directly"
    );
    Ok(Sources {
        template: template_facts(template)?,
        defaults: default_facts(generator, availability)?,
        handler_hide: parameter(
            generator,
            handler_params,
            0,
            "hideInMobiGlas",
            "HideInMobiGlas",
            "handler",
            &Value::Bool(true),
        )?,
        shadowed_hide: datum(
            generator,
            field(availability, "hideInMobiGlas")?,
            &Value::Bool(false),
        )?,
        parent: parent_facts(generator, parent_params)?,
        sub_share: parameter(
            generator,
            child,
            0,
            "canBeShared",
            "canbeshared",
            "subContract",
            &Value::Bool(false),
        )?,
        bare_template: template_link(generator, bare, template)?,
        parent_template: template_link(generator, parent, template)?,
    })
}

fn template_facts(tree: &RecordTree<'_>) -> TestResult<[Fact; 2]> {
    let class = structure(field(tree.root(), "contractClass")?)?;
    let settings = element(class, "additionalParams", 0)?;
    assert_eq!(
        settings.type_name(),
        "ActiveContractSettings",
        "the selected fallback is the physical settings entry"
    );
    let display = structure(field(tree.root(), "contractDisplayInfo")?)?;
    Ok([
        fact(
            tree,
            settings,
            "canBeShared",
            "template",
            &Value::Bool(true),
        )?,
        fact(tree, display, "illegal", "template", &Value::Bool(false))?,
    ])
}

fn default_facts<'source>(
    tree: &RecordTree<'source>,
    availability: &StructValue<'source>,
) -> TestResult<[Fact; 5]> {
    let base = |name, value| fact(tree, availability, name, "handler", value);
    Ok([
        base("onceOnly", &Value::Bool(true))?,
        base("maxPlayersPerInstance", &Value::Int32(1))?,
        base("availableInPrison", &Value::Bool(false))?,
        base("canReacceptAfterAbandoning", &Value::Bool(true))?,
        base("canReacceptAfterFailing", &Value::Bool(false))?,
    ])
}

fn parent_facts<'source>(
    tree: &RecordTree<'source>,
    overrides: &StructValue<'source>,
) -> TestResult<[Fact; 3]> {
    Ok([
        parameter(
            tree,
            overrides,
            0,
            "onceOnly",
            "OnceOnly",
            "contract",
            &Value::Bool(false),
        )?,
        parameter(
            tree,
            overrides,
            1,
            "illegal",
            "Illegal",
            "contract",
            &Value::Bool(true),
        )?,
        parameter(
            tree,
            overrides,
            0,
            "maxPlayersPerInstance",
            "MaxPlayersPerInstance",
            "contract",
            &Value::Int32(4),
        )?,
    ])
}

fn template_link<'source>(
    tree: &RecordTree<'source>,
    contract: &StructValue<'source>,
    template: &RecordTree<'_>,
) -> TestResult<Site> {
    let value = field(contract, "template")?;
    let Value::Reference(reference) = value else {
        return Err("the original contract has a resolved Template selector".into());
    };
    let record = reference.original_target().record()?;
    assert!(
        template.is_from(&record),
        "the authored reference selects the exact fallback Template Record"
    );
    site(tree, value)
}

fn parameter<'source>(
    tree: &RecordTree<'source>,
    overrides: &StructValue<'source>,
    slot: usize,
    name: &'static str,
    authored_name: &str,
    layer: &'static str,
    expected: &Value<'_>,
) -> TestResult<Fact> {
    let array = match expected {
        Value::Bool(_) => "boolParamOverrides",
        Value::Int32(_) => "intParamOverrides",
        _ => return Err("the test fixture uses Bool and Int32 overrides".into()),
    };
    let original = element(overrides, array, slot)?;
    let selector = datum(
        tree,
        field(original, "param")?,
        &Value::String(authored_name),
    )?;
    Ok(Fact {
        field: name,
        selector,
        ..fact(tree, original, "value", layer, expected)?
    })
}

fn fact<'source>(
    tree: &RecordTree<'source>,
    owner: &StructValue<'source>,
    name: &'static str,
    layer: &'static str,
    expected: &Value<'_>,
) -> TestResult<Fact> {
    let payload = datum(tree, field(owner, name)?, expected)?;
    let value = match expected {
        Value::Bool(value) => json!(value),
        Value::Int32(value) => json!(value),
        _ => return Err("the test fixture uses Bool and Int32 data".into()),
    };
    Ok(Fact {
        field: name,
        layer,
        value,
        payload,
        selector: payload,
    })
}

fn datum<'source>(
    tree: &RecordTree<'source>,
    value: &Value<'source>,
    expected: &Value<'_>,
) -> TestResult<Site> {
    assert_eq!(
        value, expected,
        "the physical fixture stores the expected input"
    );
    site(tree, value)
}

fn element<'read, 'source>(
    owner: &'read StructValue<'source>,
    name: &str,
    slot: usize,
) -> TestResult<&'read StructValue<'source>> {
    let Value::Array(values) = field(owner, name)? else {
        return Err(format!("expected the original array {name}").into());
    };
    structure(
        values
            .get(slot)
            .ok_or("missing original array occurrence")?,
    )
}

fn field<'read, 'source>(
    owner: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read Value<'source>> {
    owner
        .field(name)
        .ok_or_else(|| format!("missing original field {name}").into())
}

fn structure<'read, 'source>(
    value: &'read Value<'source>,
) -> TestResult<&'read StructValue<'source>> {
    let Value::Struct(value) = value else {
        return Err("expected an original Struct occurrence".into());
    };
    Ok(value)
}

fn site<'source>(tree: &RecordTree<'source>, selected: &Value<'source>) -> TestResult<Site> {
    let mut pending: Vec<_> = tree.fields().iter().rev().map(Field::value).collect();
    let mut ordinal = 0;
    while let Some(value) = pending.pop() {
        if std::ptr::eq(value, selected) {
            return Ok(Site {
                record: tree.record_index(),
                ordinal,
            });
        }
        ordinal += 1;
        match value {
            Value::Struct(value) => pending.extend(value.fields().iter().rev().map(Field::value)),
            Value::Array(values) => pending.extend(values.iter().rev()),
            _ => {}
        }
    }
    Err("the selected value belongs to another physical tree".into())
}

#[path = "facts_layered_file_name.rs"]
mod file_name;
