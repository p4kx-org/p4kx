// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForge};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestValue,
};
use serde_json::{Value, json};

use crate::blueprint_category_v3_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::commodity_v3_source_inventory::{self as origins, SourceField};
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

fn blueprint(catalog: &Value, guid: u128) -> TestResult<&Value> {
    let expected = common::guid(guid);
    fixture::at(catalog, "/blueprints")?
        .as_array()
        .ok_or("unmigrated Blueprint rows")?
        .iter()
        .find(|row| row.get("guid").and_then(Value::as_str) == Some(expected.as_str()))
        .ok_or_else(|| "the unchanged selected Blueprint is absent".into())
}

fn assert_backfill(catalog: &Value, magazine_index: u32) -> TestResult {
    let iron = common::definition(catalog, crafting_fixture::RESOURCE_IRON)?;
    assert_eq!(
        fixture::at(iron, "/selection/value")?,
        &json!("resourceFallback"),
        "the cost-used resource still supplies a fallback Commodity"
    );
    let uses = origins::uses(catalog, fixture::at(iron, "/selection")?)?;
    assert!(uses.iter().any(|(index, field, role)| {
        *index == u64::from(magazine_index)
            && matches!(field, SourceField::Body(path) if path.starts_with("blueprint.tiers[0].recipe.costs.") && path.ends_with(".resource"))
            && role == "selector"
    }), "Commodity backfill keeps an actual original Blueprint resource-use selector");
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
    let magazine = blueprint(catalog, crafting_fixture::MAGAZINE)?;
    assert_eq!(
        fixture::at(magazine, "/craftTimeSeconds")?,
        &json!(93_784),
        "the existing whole-second interpretation is retained"
    );
    assert_eq!(
        fixture::at(magazine, "/ingredients/0/quantityScu")?,
        &json!(0.5),
        "the first resource quantity is unchanged"
    );
    assert_eq!(
        fixture::at(magazine, "/ingredients/0/slot")?,
        &json!("MAGAZINE"),
        "nearest selection name still supplies the flattened ingredient"
    );
    assert_eq!(
        fixture::at(magazine, "/categoryGuid")?,
        &json!(common::guid(crafting_fixture::CATEGORY)),
        "a retained category link keeps its GUID"
    );
    assert_eq!(
        fixture::at(blueprint(catalog, ORPHAN)?, "/categoryGuid")?,
        &Value::Null,
        "the existing leftover-category unlink projection remains unchanged"
    );
    Ok(())
}
