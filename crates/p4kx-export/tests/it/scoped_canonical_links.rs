// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::loot_v3_graph as raw;
use crate::mission_template_graph::{self as source, Property};
use crate::scoped_canonical::{self, OWNERS, Owner};
use crate::scoped_fixture::TestResult;

fn status<'catalog>(
    property: &Property<'catalog>,
    node: &Value,
    expected: &str,
) -> Result<&'catalog Value, Box<dyn std::error::Error>> {
    let binding = property.binding(node)?;
    assert_eq!(
        raw::present(
            binding
                .get("missionPropertyStatus")
                .ok_or("actual strict weak status")?
        )?,
        &json!(expected),
        "the established weak MissionProperty decision remains contextual"
    );
    Ok(binding)
}

pub(super) fn array(catalog: &Value) -> TestResult {
    for owner in OWNERS {
        check_array(catalog, owner)?;
    }
    Ok(())
}

fn check_array(catalog: &Value, owner: Owner) -> TestResult {
    let property = owner.property(catalog)?;
    let filter = property.field(property.node, "otherLocationFilter")?;
    property.filter_kind(filter, "notLocations")?;
    raw::structure(filter, "LocationCondition_NotLocations")?;
    let links = property.field(filter, "locations")?;
    assert_eq!(
        raw::elements(links)?.len(),
        6,
        "every original weak slot survives"
    );
    for index in [0, 2] {
        let node = property.element(links, index)?;
        scoped_canonical::resolved(owner, &property, node)?;
    }
    raw::source_null(property.element(links, 1)?);
    assert_ne!(
        property.element(links, 0)?.get("sourceOrdinal"),
        property.element(links, 2)?.get("sourceOrdinal"),
        "repeated weak targets preserve separate original positions"
    );
    outside(&property, property.element(links, 3)?)?;
    foreign(&property, property.element(links, 4)?)?;
    missing(&property, property.element(links, 5)?)?;
    Ok(())
}

fn outside(property: &Property<'_>, node: &Value) -> TestResult {
    status(property, node, "outsideOwner")?;
    assert_eq!(
        node.pointer("/value/state"),
        Some(&json!("unreadable")),
        "an outside instance is not admitted as a local payload"
    );
    let error = node
        .get("originalError")
        .ok_or("actual outside-owner decoder fact")?;
    assert_eq!(
        error.get("type"),
        Some(&json!("outsideRecord")),
        "the original outside-owner failure is preserved"
    );
    for field in ["structIndex", "instanceIndex"] {
        assert!(
            raw::present(
                error
                    .get(field)
                    .ok_or("original outside instance coordinate")?
            )?
            .is_u64(),
            "unresolved original instance coordinates remain exact numeric facts"
        );
    }
    assert!(
        error.get("recordGuid").is_none() && error.get("targetRecordIndex").is_none(),
        "an unowned instance does not fabricate a Record identity or index"
    );
    Ok(())
}

fn foreign(property: &Property<'_>, node: &Value) -> TestResult {
    let binding = status(property, node, "outsideOwner")?;
    assert_eq!(
        binding.pointer("/target/interpretation"),
        Some(&json!("outsideOwner")),
        "a foreign owner is never resolved as a local property"
    );
    assert_eq!(
        binding.pointer("/target/field"),
        Some(&Value::Null),
        "no same-named local variable field is substituted"
    );
    let target = source::scalar(node, "weakReference")?;
    assert_eq!(
        raw::present(target.get("recordGuid").ok_or("actual foreign GUID")?)?,
        &json!("33000000-0000-4000-8000-000000000001"),
        "the actual known foreign owner stays distinct"
    );
    assert_eq!(
        raw::present(
            target
                .get("targetRecordIndex")
                .ok_or("actual foreign index")?
        )?,
        &json!(0),
        "the actual outside-owner physical Record position survives"
    );
    assert_eq!(
        raw::present(target.get("recordPath").ok_or("actual foreign path")?)?,
        &json!("libs/foundry/records/properties/foreign"),
        "the original foreign source path survives"
    );
    assert_eq!(
        raw::present(
            target
                .pointer("/target/sourceType")
                .ok_or("actual foreign type")?
        )?,
        &json!("MissionProperty"),
        "the original foreign target type survives"
    );
    assert_eq!(
        target.pointer("/target/type"),
        Some(&json!("record")),
        "a Record target has no fabricated owned-instance location"
    );
    assert!(
        target.pointer("/target/location").is_none(),
        "the source does not claim an owner-local instance path"
    );
    Ok(())
}

fn missing(property: &Property<'_>, node: &Value) -> TestResult {
    status(property, node, "missingTarget")?;
    assert_eq!(
        node.pointer("/value/state"),
        Some(&json!("unreadable")),
        "a missing target is not a successful null pointer"
    );
    let error = node
        .get("originalError")
        .ok_or("actual malformed pointer facts")?;
    assert_eq!(
        error.get("type"),
        Some(&json!("invalidPointer")),
        "the exact original pointer failure category remains typed"
    );
    assert_eq!(
        raw::present(
            error
                .get("structIndex")
                .ok_or("actual invalid Struct coordinate")?
        )?,
        &json!(u32::MAX - 1),
        "the authored invalid Struct index survives exactly"
    );
    assert_eq!(
        raw::present(
            error
                .get("instanceIndex")
                .ok_or("actual invalid instance coordinate")?
        )?,
        &json!(7),
        "the authored invalid instance index survives exactly"
    );
    Ok(())
}

pub(super) fn foreign_owner(catalog: &Value) -> TestResult {
    for owner in OWNERS {
        check_foreign_owner(catalog, owner)?;
    }
    Ok(())
}

fn check_foreign_owner(catalog: &Value, owner: Owner) -> TestResult {
    let property = owner.property(catalog)?;
    let filter = property.field(property.node, "otherLocationFilter")?;
    property.filter_kind(filter, "sameLocation")?;
    foreign(&property, property.field(filter, "location")?)
}

pub(super) fn other_target(catalog: &Value) -> TestResult {
    let property = Owner::Template.property(catalog)?;
    let filter = property.field(property.node, "otherLocationFilter")?;
    let node = property.field(filter, "location")?;
    status(&property, node, "unsupportedTargetType")?;
    let target = source::scalar(node, "weakReference")?;
    assert_eq!(
        raw::present(
            target
                .pointer("/target/sourceType")
                .ok_or("actual non-property type")?
        )?,
        &json!("OtherTarget"),
        "the non-property target keeps its original concrete type"
    );
    assert_eq!(
        raw::present(
            target
                .pointer("/target/location")
                .ok_or("actual non-property location")?
        )?,
        &json!("wrongTarget"),
        "the owner-local non-property source path survives"
    );
    let binding = property.binding(node)?;
    let raw_name = property.bound(
        binding
            .pointer("/target/field")
            .ok_or("actual generic target field")?,
    )?;
    assert_eq!(
        source::text(raw_name)?,
        "MustNotResolve",
        "the raw name stays data while strict MissionProperty status refuses it"
    );
    Ok(())
}

pub(super) fn unreadable_variable(catalog: &Value) -> TestResult {
    for owner in OWNERS {
        check_unreadable_variable(catalog, owner)?;
    }
    Ok(())
}

fn check_unreadable_variable(catalog: &Value, owner: Owner) -> TestResult {
    let property = owner.property(catalog)?;
    let filter = property.field(property.node, "otherLocationFilter")?;
    let node = property.field(filter, "location")?;
    let binding = status(&property, node, "unreadableTarget")?;
    let variable = property.bound(
        binding
            .pointer("/target/field")
            .ok_or("actual unreadable target field")?,
    )?;
    assert_eq!(
        variable.pointer("/value/state"),
        Some(&json!("unreadable")),
        "unreadable target text is not absence"
    );
    assert_eq!(
        variable.pointer("/originalError/type"),
        Some(&json!("invalidString")),
        "the source error remains exact and typed"
    );
    assert_eq!(
        raw::present(
            variable
                .pointer("/originalError/offset")
                .ok_or("actual invalid target string offset")?
        )?,
        &json!(u32::MAX),
        "the original unreadable string coordinate survives"
    );
    owner.assert_target(node)
}

pub(super) fn dangling_definition(catalog: &Value) -> TestResult {
    for owner in OWNERS {
        check_dangling_definition(catalog, owner)?;
    }
    Ok(())
}

fn check_dangling_definition(catalog: &Value, owner: Owner) -> TestResult {
    let property = owner.property(catalog)?;
    let filter = property.field(property.node, "locationClassFilter")?;
    property.filter_kind(filter, "excludedLocationClasses")?;
    let definitions = property.field(filter, "locationClasses")?;
    assert_eq!(
        raw::elements(definitions)?.len(),
        3,
        "every original definition slot survives"
    );
    source::reference(
        property.element(definitions, 0)?,
        "44000000-0000-4000-8000-0000000000ff",
        None,
    )?;
    raw::source_null(property.element(definitions, 1)?);
    source::reference(
        property.element(definitions, 2)?,
        "44000000-0000-4000-8000-000000000001",
        Some(4),
    )?;
    Ok(())
}
