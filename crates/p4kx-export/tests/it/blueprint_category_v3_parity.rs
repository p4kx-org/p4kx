// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::catalog_crafting_support::blueprint;
use p4kx_extraction::{Archive, DataForge};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestValue,
};
use serde_json::{Value, json};

use crate::blueprint_category_v3_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::crafting_fixture;

const ORPHAN: u128 = 0x705;

#[test]
fn category_v3_preserves_blueprint_ingredients_legacy_unlink_and_original_commodity_backfill()
-> TestResult {
    let source = crafting_fixture::database().record(
        TestRecord::new(
            ORPHAN,
            "CraftingBlueprintRecord",
            "CraftingBlueprintRecord.CategoryOrphan",
            "libs/foundry/records/crafting/blueprints/crafting/category_orphan.xml",
        )
        .value(
            "blueprint",
            TestValue::Struct(TestInstance::new("CraftingBlueprint").value(
                "category",
                TestValue::Reference(crafting_fixture::CATEGORY_LEFTOVER),
            )),
        ),
    );
    let (category_index, magazine_index) = original_crafting(&source)?;
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("categories-with-crafting.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", source.build()).encrypted())
        .entry(crafting_fixture::english())
        .entry(crafting_fixture::german())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let built = p4kx_export::build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_blueprints(&catalog)?;
    assert_backfill(&catalog, magazine_index)?;
    fixture::counts(&built, 2, 1)?;
    fixture::row(&catalog, u64::from(category_index))?;
    Ok(())
}

fn source_index(forge: &DataForge<'_>, guid: u128) -> TestResult<u32> {
    let expected = common::guid(guid);
    let mut matches = forge
        .records()
        .filter(|record| record.guid().to_string() == expected);
    let index = matches.next().ok_or("original fixture identity")?.index();
    assert!(
        matches.next().is_none(),
        "this fixture identity has exactly one original physical Record"
    );
    Ok(index)
}

fn assert_backfill(catalog: &Value, magazine_index: u32) -> TestResult {
    let iron = common::definition(catalog, crafting_fixture::RESOURCE_IRON)?;
    assert_eq!(
        fixture::at(iron, "/selection/value")?,
        &json!("resourceFallback"),
        "the cost-used resource still supplies a fallback Commodity"
    );
    let uses =
        blueprint::selected_ordinals(catalog, &iron["selection"], u64::from(magazine_index))?;
    let blueprint = blueprint::by_guid(catalog, crafting_fixture::MAGAZINE)?;
    let mut matched = 0;
    for tier in blueprint.tiers()? {
        for cost in tier["costs"].as_array().ok_or("typed costs")? {
            if cost
                .pointer("/meaning/resourceGuid/value")
                .and_then(Value::as_str)
                != Some(common::guid(crafting_fixture::RESOURCE_IRON).as_str())
            {
                continue;
            }
            let source = blueprint.node(&cost["source"])?;
            let resource = blueprint::field_ref(source, "resource")?;
            let ordinal = resource["valueIndex"]
                .as_str()
                .ok_or("actual resource ordinal")?;
            assert!(
                uses.contains(ordinal),
                "the genuine resource slot remains a backfill selector"
            );
            matched += 1;
        }
    }
    assert!(
        matched > 0,
        "the selected original Blueprint actually uses this resource"
    );
    Ok(())
}

fn original_crafting(source: &DataForgeBuilder) -> TestResult<(u32, u32)> {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let category_index = source_index(&forge, crafting_fixture::CATEGORY)?;
    fixture::original(
        source,
        category_index,
        crafting_fixture::CATEGORY,
        "BlueprintCategoryRecord.Ammo",
        "libs/foundry/records/crafting/blueprintcategories/blueprintcategorydatabase.xml",
        "",
    )?;
    fixture::fieldless(source, category_index)?;
    let magazine_index = source_index(&forge, crafting_fixture::MAGAZINE)?;
    assert_eq!(
        forge
            .record(magazine_index)
            .ok_or("original Magazine blueprint")?
            .type_name()?,
        "CraftingBlueprintRecord",
        "the backfill producer is the original Blueprint Record"
    );
    Ok((category_index, magazine_index))
}

fn assert_blueprints(catalog: &Value) -> TestResult {
    let magazine = blueprint::by_guid(catalog, crafting_fixture::MAGAZINE)?;
    let tier = magazine
        .tiers()?
        .first()
        .copied()
        .ok_or("original Magazine tier")?;
    assert_eq!(
        common::at(tier, "/craftTimeSeconds/value")?,
        "93784",
        "the source tier duration survives"
    );
    let (ingredient, slot) = magazine.ingredient(0)?;
    assert_eq!(
        common::at(ingredient, "/meaning/quantityScu/value")?,
        0.5,
        "the resource cost keeps its original quantity"
    );
    assert_eq!(
        magazine.debug_name(slot.ok_or("nearest Magazine slot")?)?,
        "MAGAZINE",
        "the resource retains its nearest named slot"
    );
    assert_eq!(
        common::at(magazine.presentation, "/category/value")?.as_str(),
        Some(common::guid(crafting_fixture::CATEGORY).as_str()),
        "the presentation links its original Blueprint category"
    );
    let orphan = blueprint::by_guid(catalog, ORPHAN)?;
    assert_eq!(
        common::at(orphan.presentation, "/category/state")?,
        "absent",
        "the existing leftover-category unlink projection remains"
    );
    Ok(())
}
