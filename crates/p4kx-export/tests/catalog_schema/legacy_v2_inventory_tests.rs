// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use serde_json::json;

use super::legacy_v2_inventory::{Address, Inventory, released_schema};
use super::schema_navigation::escape;

type Result = std::result::Result<(), Box<dyn Error>>;

#[test]
fn frozen_v2_inventory_reaches_every_root_field() -> Result {
    let schema = released_schema()?;
    let inventory = Inventory::collect(&schema)?;
    let names = schema["properties"]
        .as_object()
        .ok_or("v2 root properties")?;
    let expected: BTreeSet<_> = names
        .keys()
        .map(|name| format!("/{}", escape(name)))
        .collect();
    let actual: BTreeSet<_> = inventory
        .fields
        .iter()
        .filter(|field| field.wire.matches('/').count() == 1)
        .map(|field| field.wire.clone())
        .collect();
    assert_eq!(expected.len(), 53, "released root field count");
    assert_eq!(actual, expected, "released root coverage");
    assert_eq!(inventory.fields.len(), 2926, "released field occurrences");
    assert_eq!(inventory.recursion.len(), 10, "released recursive edges");
    Ok(())
}

#[test]
fn contract_discriminators_keep_distinct_tagged_branch_addresses() -> Result {
    let inventory = Inventory::collect(&released_schema()?)?;
    let wire = "/contractTemplates/[]/contractProperties/[]/value/type";
    let source = concat!(
        "/properties/contractTemplates/items/$ref/properties/contractProperties/items/$ref",
        "/properties/value/anyOf/0/$ref/oneOf"
    );
    let actual: BTreeSet<_> = inventory
        .fields
        .iter()
        .filter(|field| field.wire == wire)
        .map(|field| field.schema.clone())
        .collect();
    let expected = (0..19)
        .map(|branch| format!("{source}/{branch}/properties/type"))
        .collect();
    assert_eq!(
        actual, expected,
        "one declaration per released property tag"
    );
    Ok(())
}

#[test]
fn recursive_flow_edges_leave_distinct_branches_and_sibling_fields() -> Result {
    let inventory = Inventory::collect(&released_schema()?)?;
    let wire = "/contractTemplates/[]/missionFlow/[]/condition";
    let source = concat!(
        "/properties/contractTemplates/items/$ref/properties/missionFlow/items/$ref",
        "/properties/condition/anyOf/0/$ref/oneOf"
    );
    for (branch, field, path) in [
        (4, "conditions/[]", "conditions/items/$ref"),
        (5, "conditions/[]", "conditions/items/$ref"),
        (6, "condition", "condition/anyOf/0/$ref"),
    ] {
        assert!(
            inventory.recursion.contains(&Address {
                wire: format!("{wire}/{field}"),
                schema: format!("{source}/{branch}/properties/{path}"),
            }),
            "recursive flow branch {branch}"
        );
        assert!(
            inventory.fields.contains(&Address {
                wire: format!("{wire}/type"),
                schema: format!("{source}/{branch}/properties/type"),
            }),
            "flow discriminator from branch {branch}"
        );
    }
    Ok(())
}

#[test]
fn unsupported_structure_at_a_reference_is_not_silently_skipped() -> Result {
    let mut schema = released_schema()?;
    schema["properties"]["contracts"]["items"]["patternProperties"] = json!({
        ".*": { "properties": { "hiddenField": { "type": "string" } } }
    });
    let error = Inventory::collect(&schema).expect_err("unsupported shape must fail inventory");
    assert_eq!(
        error.to_string(),
        "unsupported schema keyword patternProperties at #/properties/contracts/items"
    );
    Ok(())
}
