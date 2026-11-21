// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Cost, Filter, Kind};
use p4kx_extraction::{DataForge, DataForgeEntry, Guid, StructValue, Value};

use crate::catalog_crafting_support::blueprint;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::crafting_detail;
use crate::crafting_detail_fixture as fixture;

const LEVELS: usize = 33;

#[test]
fn standalone_crafting_keeps_a_readable_blueprint_beyond_thirty_two_selections() -> TestResult {
    let bytes = fixture::nested_database(LEVELS).build();
    let forge = DataForge::open(&bytes)?;
    let original = original_blueprint(&forge)?;
    let crafting = p4kx_export::read_crafting(&forge, None, None)?;
    let blueprint = crafting
        .blueprints
        .iter()
        .find(|blueprint| blueprint.guid == original)
        .ok_or("the fully readable deep blueprint must survive standalone crafting")?;
    assert_eq!(
        blueprint.produced_item,
        Some(crafting_detail::guid(fixture::PRODUCT_A)),
        "the same original produced-item link remains on the retained blueprint"
    );
    assert!(
        crafting
            .failures
            .iter()
            .all(|failure| failure.guid != original),
        "valid original depth does not create a failed Record"
    );
    let [tier] = blueprint.tiers.as_slice() else {
        return Err("one original tier".into());
    };
    assert_eq!(tier.costs().len(), LEVELS + 1);
    let mut cost = tier
        .mandatory_cost()
        .ok_or("the original mandatory selection")?;
    for level in (0..LEVELS).rev() {
        let Cost::Select(selection) = cost else {
            return Err("the next authored selection".into());
        };
        assert_eq!(selection.count, Some(1));
        assert_eq!(selection.slot, Some(format!("LEVEL{level}").as_str()));
        let [next] = selection.options.as_slice() else {
            return Err("one actual child".into());
        };
        cost = tier.cost(*next).ok_or("same-tier cost reference")?;
    }
    let Cost::Resource(resource) = cost else {
        return Err("terminal resource".into());
    };
    assert_eq!(
        resource.resource,
        Some(crafting_detail::guid(fixture::IRON))
    );
    assert_eq!(resource.quantity_scu, Some(1.0));
    Ok(())
}

#[test]
fn normal_catalog_emits_the_readable_deep_blueprint_in_schema_three() -> TestResult {
    let source = fixture::nested_database(LEVELS);
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let original = original_blueprint(&forge)?.to_string();
    let filter = Filter::new("*/bp_craft_too_deep");
    let mut options = common::options();
    options.filter = Some(&filter);
    let built = p4kx_export::build_catalog_from(&archive, &entry, &options)?;
    assert!(
        built.summary().is_complete(),
        "the selected deep source produces complete health"
    );
    assert_eq!(
        built
            .summary()
            .kind(Kind::Blueprints)
            .ok_or("actual blueprint summary")?
            .count,
        1,
        "the one selected physical blueprint is counted as emitted"
    );
    assert!(
        built
            .summary()
            .failures()
            .iter()
            .all(|failure| failure.guid() != original.as_str()),
        "the actually selected Record is not classified as a failure"
    );
    let json: serde_json::Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        json.get("schemaVersion")
            .and_then(serde_json::Value::as_u64),
        Some(3),
        "the normal writer emits the canonical schema version"
    );
    assert_wire_chain(&json)
}

fn assert_wire_chain(json: &serde_json::Value) -> TestResult {
    let blueprint = blueprint::by_guid(json, fixture::TOO_DEEP)?;
    assert!(
        json.get("blueprints").is_none(),
        "the Blueprint appears only in the canonical tables"
    );
    let tiers = blueprint.tiers()?;
    let [tier] = tiers.as_slice() else {
        return Err("one original tier".into());
    };
    assert_eq!(
        tier["costs"].as_array().map(Vec::len),
        Some(LEVELS + 1),
        "every level plus its final resource is retained"
    );
    let mut cost = blueprint::cost(tier, &tier["mandatory"])?;
    for level in (0..LEVELS).rev() {
        let expected = format!("LEVEL{level}");
        assert_eq!(
            blueprint.debug_name(cost)?,
            expected.as_str(),
            "selection names survive in authored nesting order"
        );
        let meaning = &cost["meaning"];
        assert_eq!(
            meaning["type"], "select",
            "every nested node is an authored selection"
        );
        assert_eq!(
            common::at(meaning, "/count/value")?,
            "1",
            "each selection retains its count"
        );
        assert_eq!(
            common::at(meaning, "/count/containsEncryptedSource")?,
            true,
            "each count retains the encrypted source marker"
        );
        let options = meaning["options"].as_array().ok_or("options")?;
        let [next] = options.as_slice() else {
            return Err("one actual child".into());
        };
        cost = blueprint::cost(tier, next)?;
    }
    assert_terminal_cost(&blueprint, cost)
}

fn assert_terminal_cost(
    blueprint: &blueprint::Blueprint<'_>,
    cost: &serde_json::Value,
) -> TestResult {
    assert_eq!(
        common::at(cost, "/meaning/type")?,
        "resource",
        "the deepest cost is the original resource"
    );
    assert_eq!(
        common::at(cost, "/meaning/resourceGuid/value")?.as_str(),
        Some(common::guid(fixture::IRON).as_str()),
        "the deepest cost keeps the source resource GUID"
    );
    assert_eq!(
        common::at(cost, "/meaning/quantityScu/value")?,
        1.0,
        "the deepest cost keeps its original SCU quantity"
    );
    let (ingredient, slot) = blueprint.ingredient(0)?;
    assert_eq!(
        ingredient["source"], cost["source"],
        "the derived ingredient references the deepest original resource"
    );
    assert_eq!(
        blueprint.debug_name(slot.ok_or("nearest deep selection")?)?,
        "LEVEL0",
        "the derived ingredient references the nearest original slot"
    );
    Ok(())
}

fn original_blueprint(forge: &DataForge<'_>) -> TestResult<Guid> {
    let guid = crafting_detail::guid(fixture::TOO_DEEP);
    let record = forge
        .record_by_guid(guid)
        .ok_or("the original deep blueprint")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the independently decoded Tree belongs to the actual Record"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "the original source has no unreadable slots"
    );
    let blueprint = structure(tree.root(), "blueprint")?;
    let tier = single_structure(blueprint, "tiers")?;
    let recipe = structure(tier, "recipe")?;
    let costs = structure(recipe, "costs")?;
    let mut cost = structure(costs, "mandatoryCost")?;
    for level in (0..LEVELS).rev() {
        assert_eq!(
            cost.type_name(),
            "CraftingCost_Select",
            "the actual ordered source selection at level {level}"
        );
        assert_eq!(
            cost.field("count"),
            Some(&Value::Int32(1)),
            "each original selection has its actual count"
        );
        let name = structure(cost, "nameInfo")?;
        let expected = format!("LEVEL{level}");
        assert_eq!(
            name.field("debugName"),
            Some(&Value::String(&expected)),
            "the source preserves the original outer-to-inner slot order"
        );
        cost = single_structure(cost, "options")?;
    }
    assert_eq!(
        cost.type_name(),
        "CraftingCost_Resource",
        "the entire chain reaches its actual terminal resource"
    );
    let Some(Value::Reference(resource)) = cost.field("resource") else {
        return Err("the original resource leaf must keep its typed Reference".into());
    };
    assert_eq!(
        resource.guid(),
        crafting_detail::guid(fixture::IRON),
        "the exact original Resource GUID is readable"
    );
    let quantity = structure(cost, "quantity")?;
    assert_eq!(
        quantity.field("standardCargoUnits"),
        Some(&Value::Single(1.0)),
        "the actual source leaf has one SCU"
    );
    Ok(record.guid())
}

fn structure<'read, 'source>(
    parent: &'read StructValue<'source>,
    field: &str,
) -> TestResult<&'read StructValue<'source>> {
    match parent.field(field) {
        Some(Value::Struct(value)) => Ok(value),
        _ => Err(format!("the original {field} is a struct").into()),
    }
}

fn single_structure<'read, 'source>(
    parent: &'read StructValue<'source>,
    field: &str,
) -> TestResult<&'read StructValue<'source>> {
    if let Some(Value::Array(values)) = parent.field(field)
        && let [Value::Struct(value)] = values.as_ref()
    {
        return Ok(value);
    }
    Err(format!("the original {field} holds one struct at its actual first slot").into())
}
