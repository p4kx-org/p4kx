// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_fixture::{self, TestResult};
use crate::loot_v3_graph as raw;
use crate::mission_template_graph::{self as source, Property};
use crate::scoped_fixture;

#[derive(Clone, Copy)]
pub(crate) enum Owner {
    Template,
    Broker,
}

pub(crate) const OWNERS: [Owner; 2] = [Owner::Template, Owner::Broker];

impl Owner {
    pub(crate) fn property(self, catalog: &Value) -> TestResult<Property<'_>> {
        let property = match self {
            Self::Template => Property::new(catalog, "Scoped_BP")?,
            Self::Broker => Property::broker(catalog, "Scoped_BP")?,
        };
        property.kind("scopedLocation")?;
        Ok(property)
    }

    const fn instance_location(self) -> &'static str {
        match self {
            Self::Template => "contractProperties[0]",
            Self::Broker => "properties[0]",
        }
    }

    pub(crate) fn assert_target(self, node: &Value) -> TestResult {
        let (index, guid) = match self {
            Self::Template => (1, scoped_fixture::TEMPLATE),
            Self::Broker => (2, scoped_fixture::BROKER),
        };
        let target = source::scalar(node, "weakReference")?;
        assert_eq!(
            raw::present(
                target
                    .get("targetRecordIndex")
                    .ok_or("original weak owner")?
            )?,
            &json!(index),
            "the weak target remains in the physical owning Record"
        );
        assert_eq!(
            raw::present(
                target
                    .get("recordGuid")
                    .ok_or("actual owning Record GUID")?
            )?,
            &json!(health_origin_fixture::guid(guid)),
            "the resolved target keeps its physical owner"
        );
        assert_eq!(
            raw::present(
                target
                    .pointer("/target/location")
                    .ok_or("actual owned path")?
            )?,
            &json!(self.instance_location()),
            "the original owned target path is retained"
        );
        assert!(
            raw::present(
                target
                    .get("recordPath")
                    .ok_or("original owning Record path")?
            )?
            .is_string(),
            "the resolved original owner path is retained"
        );
        assert_eq!(
            raw::present(
                target
                    .pointer("/target/sourceType")
                    .ok_or("original concrete target type")?
            )?,
            &json!("MissionProperty"),
            "the actual target type is preserved independently of the variable"
        );
        Ok(())
    }
}

const FILTERS: [&str; 6] = [
    "factionFilter",
    "locationClassFilter",
    "locationSizeFilter",
    "locationFunctionFilter",
    "otherLocationFilter",
    "relativeLocationFilter",
];

pub(crate) fn absent(catalog: &Value) -> TestResult {
    for owner in OWNERS {
        check_absent(&owner.property(catalog)?)?;
    }
    let embedded = Property::new(catalog, "Embedded_BP")?;
    check_absent(&embedded)?;
    assert!(
        embedded
            .selected
            .get("objective")
            .is_some_and(|value| !value.is_null()),
        "the selected embedded property remains attached to its actual objective"
    );
    assert_eq!(
        raw::array(embedded.projection, "objectives")?.len(),
        1,
        "the original objective remains selected in the canonical context"
    );
    Ok(())
}

fn check_absent(property: &Property<'_>) -> TestResult {
    property.kind("scopedLocation")?;
    raw::structure(property.node, "MissionPropertyValue_ScopedLocation")?;
    for field in FILTERS {
        raw::source_null(property.field(property.node, field)?);
    }
    assert!(
        raw::array(property.meaning, "scopedFilters")?.is_empty(),
        "absent filters create no fabricated contextual filter operation"
    );
    Ok(())
}

pub(crate) fn raw_enums(catalog: &Value) -> TestResult {
    for owner in OWNERS {
        check_raw_enums(catalog, owner)?;
    }
    Ok(())
}

fn check_raw_enums(catalog: &Value, owner: Owner) -> TestResult {
    let property = owner.property(catalog)?;
    let faction = property.field(property.node, "factionFilter")?;
    raw::structure(faction, "LocationCondition_ExcludedFactionHostility")?;
    property.filter_kind(faction, "excludedFactionHostility")?;
    assert_eq!(
        source::scalar(property.field(faction, "hostility")?, "enum")?,
        &json!("FutureHostility"),
        "the unknown authored hostility name is not interpreted as a host policy"
    );
    let relative = property.field(property.node, "relativeLocationFilter")?;
    raw::structure(relative, "LocationCondition_DifferentScope")?;
    property.filter_kind(relative, "differentScope")?;
    assert_eq!(
        source::scalar(property.field(relative, "scope")?, "enum")?,
        &json!("FutureScope"),
        "the authored scope name remains exact"
    );
    resolved(owner, &property, property.field(relative, "location")?)
}

pub(crate) fn empty(catalog: &Value) -> TestResult {
    for owner in OWNERS {
        check_empty(catalog, owner)?;
    }
    Ok(())
}

fn check_empty(catalog: &Value, owner: Owner) -> TestResult {
    let property = owner.property(catalog)?;
    let class = property.field(property.node, "locationClassFilter")?;
    raw::structure(class, "LocationCondition_RequiredLocationClasses")?;
    property.filter_kind(class, "requiredLocationClasses")?;
    assert!(
        raw::elements(property.field(class, "locationClasses")?)?.is_empty(),
        "a declared empty definition array stays distinct from an absent filter"
    );
    let function = property.field(property.node, "locationFunctionFilter")?;
    raw::structure(function, "LocationCondition_ExcludedLocationFunction")?;
    property.filter_kind(function, "excludedLocationFunction")?;
    raw::source_null(property.field(function, "locationFunction")?);
    raw::source_null(property.field(property.node, "locationSizeFilter")?);
    Ok(())
}

pub(crate) fn resolved(owner: Owner, property: &Property<'_>, node: &Value) -> TestResult {
    let binding = property.binding(node)?;
    assert_eq!(
        raw::present(
            binding
                .get("missionPropertyStatus")
                .ok_or("actual strict property status")?
        )?,
        &json!("resolved"),
        "the original same-owner weak link resolves as a MissionProperty"
    );
    let target = binding.get("target").ok_or("actual variable binding")?;
    assert_eq!(
        target.get("interpretation"),
        Some(&json!("value")),
        "the selected variable is a real source value"
    );
    let variable = property.bound(
        target
            .get("field")
            .ok_or("original variable source field")?,
    )?;
    assert_eq!(
        source::text(variable)?,
        "Origin_BP",
        "the variable comes from the actual target property"
    );
    owner.assert_target(node)
}
