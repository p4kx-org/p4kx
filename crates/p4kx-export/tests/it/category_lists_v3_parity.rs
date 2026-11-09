// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::BuiltCatalog;
use p4kx_extraction::{Archive, DataForge};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};
use serde_json::{Value, json};

use crate::blueprint_category_v3_fixture;
use crate::category_lists_v3_fixture as fixture;
use crate::category_lists_v3_source as source;
use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::crafting_fixture;

#[test]
fn category_lists_v3_preserves_existing_blueprint_category_and_commodity_backfill_outputs()
-> TestResult {
    let database = crafting_fixture::database();
    let original_index = original_owner(&database)?;
    let (built, catalog) = build(&database)?;
    fixture::count(&built, 1, 0)?;
    blueprint_category_v3_fixture::counts(&built, 2, 1)?;
    assert_eq!(
        blueprint_category_v3_fixture::rows(&catalog)?.len(),
        2,
        "the database does not count as a category definition"
    );
    fixture::row(&catalog, u64::from(original_index))?;
    assert_recipe(&catalog)?;
    let iron = common::definition(&catalog, crafting_fixture::RESOURCE_IRON)?;
    assert_eq!(
        fixture::at(iron, "/selection/value")?,
        &json!("resourceFallback"),
        "cost-used Resource fallback selection stays unchanged"
    );
    Ok(())
}

fn original_owner(database: &DataForgeBuilder) -> TestResult<u32> {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let expected = common::guid(crafting_fixture::CATEGORY_DATABASE);
    let mut originals = forge
        .records()
        .filter(|record| record.guid().to_string() == expected);
    let original = originals.next().ok_or("original crafting category list")?;
    assert!(
        originals.next().is_none(),
        "the parity fixture has one physical list owner"
    );
    assert_eq!(
        original.type_name()?,
        fixture::TYPE,
        "the original source is the separate database owner"
    );
    let slots = source::slots(database, original.index())?;
    assert_eq!(
        slots.len(),
        2,
        "the original database has two authored category slots"
    );
    for (slot, expected) in slots.iter().zip([
        crafting_fixture::CATEGORY,
        crafting_fixture::CATEGORY_ARMOUR,
    ]) {
        let source::Slot::Reference { guid, kind, .. } = slot else {
            return Err("original category Reference".into());
        };
        assert_eq!(
            guid,
            &common::guid(expected),
            "the original category listing keeps its two authored targets in order"
        );
        assert_eq!(
            kind,
            fixture::CATEGORY_TYPE,
            "the source target is an actual category Record"
        );
    }
    Ok(original.index())
}

fn build(database: &DataForgeBuilder) -> TestResult<(BuiltCatalog, Value)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("category-list-parity.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()).encrypted())
        .entry(crafting_fixture::english())
        .entry(crafting_fixture::german())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let options = common::options();
    let built = p4kx_export::build_catalog(&archive, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}

fn assert_recipe(catalog: &Value) -> TestResult {
    let guid = common::guid(crafting_fixture::MAGAZINE);
    let blueprint = fixture::at(catalog, "/blueprints")?
        .as_array()
        .ok_or("existing Blueprints")?
        .iter()
        .find(|row| row.get("guid").and_then(Value::as_str) == Some(guid.as_str()))
        .ok_or("original selected Magazine blueprint")?;
    for (path, expected) in [
        (
            "/categoryGuid",
            json!(common::guid(crafting_fixture::CATEGORY)),
        ),
        ("/craftTimeSeconds", json!(93_784)),
        ("/ingredients/0/quantityScu", json!(0.5)),
        ("/ingredients/0/slot", json!("MAGAZINE")),
    ] {
        assert_eq!(
            fixture::at(blueprint, path)?,
            &expected,
            "reading category lists leaves established Blueprint convenience values unchanged"
        );
    }
    Ok(())
}
