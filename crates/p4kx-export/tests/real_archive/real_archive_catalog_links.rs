// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::error::Error;

use p4kx_export::Kind;
use serde_json::Value;

use crate::catalog_v3_consumer;
use crate::real_archive_blueprints::Blueprints;
use crate::real_archive_broker_entries::check_broker_links;
use crate::real_archive_contract_templates::check_template_links;
use crate::real_archive_npc_spawns::check_npc_links;
use crate::real_archive_spawns::check_spawn_links;
use crate::real_archive_verification;

type TestResult = Result<(), Box<dyn Error>>;

/// Checks owned links against the selected original target identities.
pub(crate) fn check(catalog: &Value) -> TestResult {
    check_spawn_links(catalog)?;
    check_npc_links(catalog)?;
    check_template_links(catalog)?;
    check_broker_links(catalog)?;
    check_manufacturer_links(catalog)?;
    check_resource_group_links(catalog)?;
    check_blueprint_links(catalog)
}

fn check_manufacturer_links(catalog: &Value) -> TestResult {
    let manufacturers = real_archive_verification::guids(catalog, Kind::Manufacturers.name())?;
    for kind in [Kind::Items, Kind::Ships] {
        for entity in real_archive_verification::list(catalog, kind)? {
            if let Some(guid) = entity.get("manufacturerGuid").and_then(Value::as_str) {
                assert!(manufacturers.contains(guid), "{kind} manufacturer {guid}");
            }
        }
    }
    Ok(())
}

fn check_resource_group_links(catalog: &Value) -> TestResult {
    let groups = real_archive_verification::guids(catalog, Kind::ResourceGroups.name())?;
    for commodity in catalog_v3_consumer::presentations(catalog)? {
        let carrier = commodity.get("typeGuid").ok_or("Commodity type carrier")?;
        if let Some(guid) = catalog_v3_consumer::nullable_guid(carrier)? {
            assert!(groups.contains(guid), "commodity type {guid}");
        }
    }
    let mut nested = 0_usize;
    for group in real_archive_verification::list(catalog, Kind::ResourceGroups)? {
        if let Some(guid) = group.get("parentGuid").and_then(Value::as_str) {
            assert!(groups.contains(guid), "parent group {guid}");
            nested += 1;
        }
    }
    assert!(nested > 0, "at least one nested resource group");
    Ok(())
}

fn check_blueprint_links(catalog: &Value) -> TestResult {
    let items = real_archive_verification::guids(catalog, Kind::Items.name())?;
    let ships = real_archive_verification::guids(catalog, Kind::Ships.name())?;
    let producible: HashSet<_> = items.union(&ships).copied().collect();
    let categories = real_archive_verification::guids(catalog, Kind::BlueprintCategories.name())?;
    let resources = real_archive_verification::guids(catalog, Kind::ResourceTypes.name())?;
    let blueprints = Blueprints::read(catalog)?;
    for blueprint in blueprints.rows() {
        if let Some(guid) = blueprint
            .presentation
            .pointer("/category/value")
            .and_then(Value::as_str)
        {
            assert!(categories.contains(guid), "blueprint category {guid}");
        }
        if let Some(guid) = blueprint
            .presentation
            .pointer("/producedItem/value")
            .and_then(Value::as_str)
        {
            assert!(producible.contains(guid), "produced entity {guid}");
        }
        for ingredient in blueprint.ingredients()? {
            let meaning = &ingredient["meaning"];
            if let Some(guid) = meaning
                .pointer("/resourceGuid/value")
                .and_then(Value::as_str)
            {
                assert!(resources.contains(guid), "resource {guid}");
            }
            if let Some(guid) = meaning.pointer("/itemGuid/value").and_then(Value::as_str) {
                assert!(items.contains(guid), "ingredient item {guid}");
            }
        }
    }
    Ok(())
}
