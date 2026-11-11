// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceInventory, SourceLocation, SourceUseRole};
use serde_json::{Value, json};

use crate::flow_owner_graph::Owner;
use crate::flow_root_fixture::{self as fixture, Case};
use crate::flow_root_positions::Original;
use crate::health_origin_fixture::{Input, TestResult};
use crate::loot_v3_graph as raw;
use crate::loot_v3_origin::{self as origin, ExpectedOrigin, ExpectedUse};

#[path = "flow_root_coordinates.rs"]
mod coordinates;

pub(crate) fn check(
    catalog: &Value,
    inventory: &SourceInventory,
    original: &Original,
    case: Case,
    input: Input,
) -> TestResult {
    let definitions = catalog
        .pointer("/definitions/contractTemplates/rows")
        .and_then(Value::as_array)
        .ok_or("canonical Template definitions")?;
    assert_eq!(definitions.len(), 1, "one actual physical owner");
    let definition = definitions.first().ok_or("actual physical owner")?;
    assert_eq!(
        definition.pointer("/guid/value"),
        Some(&json!(fixture::GUID)),
        "actual original source identity"
    );
    let rows = catalog
        .pointer("/configurations/missionFlows/rows")
        .and_then(Value::as_array)
        .ok_or("canonical source Flow configurations")?;
    assert_eq!(rows.len(), 1, "one actual missionFlow declaration");
    let row = rows.first().ok_or("actual Flow row")?;
    check_owner(definition, row, inventory, original, case, input)
}

pub(crate) fn check_owner(
    definition: &Value,
    row: &Value,
    inventory: &SourceInventory,
    original: &Original,
    case: Case,
    input: Input,
) -> TestResult {
    assert!(
        row.get("values").is_none(),
        "C only navigates the common D inventory"
    );
    let owner = Owner::new(definition)?;
    let values = owner.members(row)?;
    union(&values, original)?;
    selected_fields(row, &owner, inventory, original, case, input)?;
    bindings(row, &owner, inventory, original, case, input)
}

fn target<'catalog>(
    owner: &Owner<'catalog>,
    field: &Value,
    ordinal: u64,
) -> TestResult<&'catalog Value> {
    let node = owner.follow(
        owner
            .field(field)?
            .get("value")
            .ok_or("selected field local reference")?,
    )?;
    assert_eq!(
        node.get("sourceOrdinal"),
        Some(&json!(ordinal.to_string())),
        "selected local root is the genuine original field occurrence"
    );
    Ok(node)
}

fn overlap(id: &Value, variable: &Value, owner: &Owner<'_>) -> TestResult {
    let fields = raw::structure(id, "NestedTarget")?;
    let first = fields
        .first()
        .ok_or("nested original property declaration")?;
    assert!(
        std::ptr::eq(
            owner.follow(
                first
                    .get("value")
                    .ok_or("nested original property reference")?
            )?,
            variable
        ),
        "overlapping roots reference one actual serialized node, not duplicate equal values"
    );
    let number = fields
        .get(1)
        .ok_or("selected original nonfinite declaration")?;
    let value = owner.follow(number.get("value").ok_or("selected numeric reference")?)?;
    assert_eq!(
        raw::payload(value, "number")?.pointer("/value/type"),
        Some(&json!("f64")),
        "actual selected Double width"
    );
    assert_eq!(
        raw::payload(value, "number")?.pointer("/value/rawBits"),
        Some(&json!("7ff800000000abcd")),
        "every selected original NaN bit"
    );
    Ok(())
}

fn bindings(
    row: &Value,
    owner: &Owner<'_>,
    inventory: &SourceInventory,
    original: &Original,
    case: Case,
    input: Input,
) -> TestResult {
    let selected = raw::array(row, "referencedFields")?;
    let values = owner.members(row)?;
    for (kind, field, interpretation, target, caller, index) in [
        (
            "tokenState",
            "objective",
            if matches!(case, Case::InsideFlow) {
                "id"
            } else {
                "wrongShape"
            },
            &original.id,
            &original.objective_caller,
            0,
        ),
        (
            "propertyIsSet",
            "property",
            "value",
            &original.variable,
            &original.property_caller,
            1,
        ),
    ] {
        let node = values
            .iter()
            .find(|node| node.pointer("/condition/type") == Some(&json!(kind)))
            .ok_or("actual condition meaning")?;
        let binding = node
            .pointer(&format!("/condition/{field}"))
            .ok_or("actual selected field binding")?;
        assert_eq!(
            binding.get("field"),
            selected.get(index),
            "typed link selects the same genuine physical field"
        );
        assert_eq!(
            binding.get("interpretation"),
            Some(&json!(interpretation)),
            "raw value remains separate from its nullable interpretation"
        );
        let uses = [
            contribution(original.ordinal(target)?, SourceUseRole::Payload),
            contribution(original.ordinal(caller)?, SourceUseRole::Selector),
            contribution(original.ordinal(caller)?, SourceUseRole::Conversion),
        ];
        origin::verify(
            binding,
            inventory,
            &ExpectedOrigin {
                encrypted: input.marker(),
                complete: true,
                uses: &uses,
            },
        )
        .map_err(|problem| format!("actual target-field selection contributors: {problem:?}"))?;
    }
    Ok(())
}

fn contribution(ordinal: u64, role: SourceUseRole) -> ExpectedUse {
    ExpectedUse {
        role,
        location: SourceLocation::Subresource {
            namespace: "dataforgeValue".to_owned(),
            indices: vec![0, ordinal],
            field: "$value".to_owned(),
        },
    }
}

fn union(values: &[&Value], original: &Original) -> TestResult {
    let actual = values
        .iter()
        .map(|node| crate::flow_owner_graph::ordinal(node))
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(
        actual,
        original.union()?,
        "physical-occurrence union, never equality interning or a whole-record dump"
    );
    Ok(())
}

fn selected_fields(
    row: &Value,
    owner: &Owner<'_>,
    inventory: &SourceInventory,
    original: &Original,
    case: Case,
    input: Input,
) -> TestResult {
    let fields = raw::array(row, "referencedFields")?;
    assert_eq!(
        fields.len(),
        2,
        "selected id and variable declarations appear exactly once"
    );
    let id = target(
        owner,
        fields.first().ok_or("selected id root")?,
        original.ordinal(&original.id)?,
    )?;
    let variable = target(
        owner,
        fields.get(1).ok_or("selected variable root")?,
        original.ordinal(&original.variable)?,
    )?;
    assert_eq!(
        raw::payload(variable, "string")?.pointer("/value/sourceText"),
        Some(&json!(fixture::VARIABLE)),
        "one exact selected variable cell"
    );
    coordinates::check(owner, fields, original, case)?;
    for (node, path) in [(id, &original.id), (variable, &original.variable)] {
        let uses = [contribution(
            original.ordinal(path)?,
            SourceUseRole::Payload,
        )];
        origin::verify(
            node.get("value").ok_or("raw own sourced cell")?,
            inventory,
            &ExpectedOrigin {
                encrypted: input.marker(),
                complete: true,
                uses: &uses,
            },
        )
        .map_err(|problem| format!("raw field keeps its own contribution: {problem:?}"))?;
    }
    if matches!(case, Case::InsideFlow) {
        assert_eq!(
            raw::payload(id, "guid")?.get("value"),
            Some(&json!(fixture::ID)),
            "actual original inside GUID"
        );
    } else {
        overlap(id, variable, owner)?;
    }
    Ok(())
}
