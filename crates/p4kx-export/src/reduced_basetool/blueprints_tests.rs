// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::ReducedError;
use super::blueprint_catalog;
use super::blueprint_recipe_coordinates::RecipeCoordinates;

use super::selected_texts::SelectedTexts;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

#[path = "blueprints_fixture.rs"]
mod fixture;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn blueprint_presentations_close_physical_wiki_tier_ingredient_and_text_joins() -> TestResult {
    with_queries(
        &fixture::database(fixture::RangeType::Linear),
        |parsed, scratch| {
            let mut texts = SelectedTexts::default();
            let blueprints = blueprint_catalog::collect(parsed, scratch, None, &mut texts)?;
            let definitions = serde_json::to_value(blueprints.definitions)?;
            let presentations = serde_json::to_value(blueprints.presentations)?;
            let tiers = serde_json::to_value(blueprints.tiers)?;
            verify_tiers(rows(&tiers)?)?;
            let processes = serde_json::to_value(blueprints.dismantle_processes)?;
            let [process] = rows(&processes)? else {
                return Err("one selected dismantle process".into());
            };
            let definitions = rows(&definitions)?;
            let presentations = rows(&presentations)?;
            let [primary, alias] = definitions else {
                return Err("both physical Blueprint aliases are exported".into());
            };
            assert_eq!(
                primary.get("guid"),
                alias.get("guid"),
                "nominal GUID matches"
            );
            assert_ne!(
                primary.get("recordIndex"),
                alias.get("recordIndex"),
                "distinct physical Records keep unique wiki joins"
            );
            assert_eq!(presentations.len(), definitions.len());
            for (definition, presentation) in definitions.iter().zip(presentations) {
                verify_owner(definition, presentation)?;
                verify_recipe(presentation)?;
                assert_eq!(
                    presentation.pointer("/dismantle/value/processRecordIndex"),
                    process.pointer("/recordIndex/value")
                );
            }
            verify_taxonomy(
                definitions,
                &serde_json::to_value(blueprints.categories)?,
                &serde_json::to_value(blueprints.pools)?,
                &serde_json::to_value(blueprints.pool_weights)?,
            )?;
            verify_texts(&serde_json::to_value(texts)?)
        },
    )
}

fn verify_taxonomy(
    definitions: &[Value],
    categories: &Value,
    pools: &Value,
    weights: &Value,
) -> TestResult {
    let [_category] = rows(categories)? else {
        return Err("one selected category".into());
    };
    let [pool] = rows(pools)? else {
        return Err("one selected pool".into());
    };
    let [weight] = rows(weights)? else {
        return Err("one selected pool weight".into());
    };
    let target = pool
        .pointer("/blueprintRewards/value/0/value/blueprintRecord/value/targetRecordIndex/value")
        .ok_or("resolved physical Blueprint")?;
    assert!(
        definitions
            .iter()
            .any(|definition| { definition.pointer("/recordIndex/value") == Some(target) })
    );
    assert_eq!(
        pool.pointer("/recordIndex/value"),
        weight.pointer("/pool/recordIndex/value")
    );
    assert_eq!(
        weight.pointer("/totalWeight/value/finite"),
        Some(&json!(2.5))
    );
    Ok(())
}

fn verify_owner(definition: &Value, presentation: &Value) -> TestResult {
    let position = definition
        .pointer("/recordIndex/value")
        .ok_or("physical definition index")?;
    assert_eq!(
        presentation.pointer("/blueprint/recordIndex"),
        Some(position)
    );
    assert!(definition.get("inputRef").is_none());
    assert!(presentation.pointer("/blueprint/inputRef").is_none());
    let class = definition
        .pointer("/className/value")
        .and_then(Value::as_str)
        .ok_or("original Wiki key")?;
    assert!(
        matches!(class, "FixturePrimary" | "FixtureAlias"),
        "retain the source casing"
    );
    assert_eq!(
        definition.pointer("/className/containsEncryptedSource"),
        Some(&json!(true))
    );
    for omitted in ["source", "additionalFields", "wikiKey"] {
        assert!(definition.get(omitted).is_none());
        assert!(presentation.get(omitted).is_none());
    }
    assert_eq!(
        presentation.pointer("/defaultAvailable/value"),
        Some(&json!(false)),
        "an absent default declaration leaves this Blueprint unavailable by default"
    );
    assert_eq!(
        presentation.pointer("/category/state"),
        Some(&json!("present"))
    );
    assert_eq!(
        presentation.pointer("/producedItem/state"),
        Some(&json!("present"))
    );
    Ok(())
}

fn verify_recipe(presentation: &Value) -> TestResult {
    assert_eq!(presentation.get("firstTier"), Some(&json!(1)));
    assert_eq!(
        presentation.pointer("/dismantle/value/tier"),
        Some(&json!(1))
    );
    let ingredients = presentation
        .get("ingredients")
        .and_then(Value::as_array)
        .ok_or("ingredients")?;
    assert_eq!(
        ingredients.as_slice(),
        &[
            json!({"cost": {"tierIndex": 1, "root": {"type": "mandatory"}, "options": []}, "slot": null}),
            json!({
                "cost": {"tierIndex": 1, "root": {"type": "optional", "index": 1}, "options": [1, 1]},
                "slot": {"tierIndex": 1, "root": {"type": "optional", "index": 1}, "options": []}
            }),
            json!({
                "cost": {"tierIndex": 1, "root": {"type": "optional", "index": 1}, "options": [2]},
                "slot": {"tierIndex": 1, "root": {"type": "optional", "index": 1}, "options": []}
            }),
        ],
        "the nearest named selection and every authored nested position close without arena indices"
    );
    Ok(())
}

fn verify_texts(texts: &Value) -> TestResult {
    let key = texts
        .pointer("/gameTexts/0/key/value")
        .ok_or("protected Blueprint title")?;
    assert_eq!(key, &json!(format!("@{}", fixture::LOCALE_KEY)));
    let localized: &str = "/gameTexts/0/translations/value";
    for (language, expected) in [("en", "Fixture Blueprint"), ("de", "Blaupause")] {
        let path = format!("{localized}/{language}/displayText/value");
        assert_eq!(
            texts.pointer(&path),
            Some(&json!(expected)),
            "{language} retained"
        );
    }
    assert_eq!(
        texts.pointer("/publicNames"),
        Some(&json!([])),
        "title stays protected"
    );
    Ok(())
}

#[test]
fn blueprint_recipe_addresses_include_later_tiers_and_reject_wrong_cost_roles() -> TestResult {
    with_queries(
        &fixture::database(fixture::RangeType::Linear),
        |parsed, _scratch| {
            parsed.with_blueprints(CatalogRecordSelection::All, None, |snapshot| {
                let [first, later, ..] = snapshot.tiers() else {
                    return Err("canonical fixture needs both authored tiers".into());
                };
                let owner_tiers = snapshot.tiers().get(..2).ok_or("the first owner's tiers")?;
                let coordinates = RecipeCoordinates::new(owner_tiers, None)?;
                assert_eq!(coordinates.tier(first.source())?, 1, "first admitted tier");
                assert_eq!(
                    coordinates.tier(later.source())?,
                    3,
                    "all owner tiers are indexed"
                );
                let later_cost = later.mandatory().ok_or("later mandatory cost")?;
                let (later_coordinate, slot) = coordinates.ingredient(later_cost, None, 3, None)?;
                assert_eq!(later_coordinate.tier_index, 3);
                assert!(slot.is_none());
                let named_selection = first
                    .optional()
                    .first()
                    .ok_or("first named optional slot")?;
                let root = first.mandatory().ok_or("first mandatory cost")?;
                for result in [
                    coordinates.ingredient(*named_selection, None, 1, None),
                    coordinates.ingredient(later_cost, None, 1, None),
                    coordinates.ingredient(root, Some(root), 1, None),
                    coordinates.ingredient(root, Some(*named_selection), 1, None),
                ] {
                    assert!(matches!(result, Err(ReducedError::IncompleteScope(_))));
                }
                Ok(())
            })?
        },
    )
}

#[test]
fn reduced_blueprint_tiers_refuse_unrepresented_modifier_ranges() -> TestResult {
    with_queries(
        &fixture::database(fixture::RangeType::Unsupported),
        |parsed, scratch| {
            let unsupported =
                parsed.with_blueprints(CatalogRecordSelection::All, None, |snapshot| {
                    snapshot.unsupported_ranges().len()
                })?;
            assert_eq!(
                unsupported, 2,
                "both original aliases contain the unsupported class"
            );
            let result =
                blueprint_catalog::collect(parsed, scratch, None, &mut SelectedTexts::default());
            assert!(
                matches!(
                    result,
                    Err(ReducedError::IncompleteScope(
                        super::blueprint_recipe_coordinates::PATH
                    ))
                ),
                "the selected range cannot disappear from a complete reduced report"
            );
            Ok(())
        },
    )
}

fn verify_tiers(tiers: &[Value]) -> TestResult {
    let [first, later, alias, alias_later] = tiers else {
        return Err("two canonical tier occurrences per physical owner".into());
    };
    for (tier, index, owner) in [
        (first, 1, 0),
        (later, 3, 0),
        (alias, 1, 1),
        (alias_later, 3, 1),
    ] {
        assert_eq!(tier.pointer("/tierIndex/value"), Some(&json!(index)));
        assert_eq!(tier.pointer("/blueprint/recordIndex"), Some(&json!(owner)));
        assert!(tier.pointer("/blueprint/inputRef").is_none());
        assert!(tier.get("source").is_none());
        assert!(tier.get("research").is_none());
        assert_eq!(tier.pointer("/craftTimeSeconds/value"), Some(&json!("75")));
    }
    assert_eq!(first.pointer("/optional/0/root/index"), Some(&json!(1)));
    assert_eq!(first.pointer("/optional/0/options"), Some(&json!([])));
    assert_eq!(
        first.get("mandatory"),
        first.pointer("/costs/0/position/value"),
        "the mandatory cost joins to its projected row"
    );
    assert_eq!(
        first.pointer("/costs/1/meaning/debugName/value"),
        Some(&json!({"type":"gameText", "sourceText":"FixtureChoice"})),
        "resolve the original protected slot debug text"
    );
    assert_eq!(
        first.pointer("/costs/1/meaning/count/value"),
        Some(&json!("1"))
    );
    assert_eq!(
        first.pointer("/costs/2/meaning/options/0/options"),
        Some(&json!([1, 1])),
        "both gaps in the nested authored option arrays survive"
    );
    assert_eq!(later.pointer("/costs/0/meaning/type"), Some(&json!("item")));
    assert_eq!(
        later.pointer("/costs/0/meaning/quantityUnits/value"),
        Some(&json!("4"))
    );
    assert_eq!(
        later.pointer("/costs/0/meaning/minQuality/value"),
        Some(&json!("200"))
    );
    verify_material(first)
}

fn verify_material(tier: &Value) -> TestResult {
    let material = tier
        .pointer("/costs/0")
        .ok_or("the mandatory material cost")?;
    assert_eq!(material.pointer("/meaning/type"), Some(&json!("resource")));
    assert_eq!(
        material.pointer("/meaning/quantityScu/value"),
        Some(&json!(0.5))
    );
    assert_eq!(
        material.pointer("/meaning/minQuality/value"),
        Some(&json!("150"))
    );
    assert_eq!(
        material.pointer("/meaning/resourceGuid/value"),
        Some(&json!("00000000-0000-0000-0000-000011777335"))
    );
    assert_eq!(
        material.pointer("/context/quantityMultiplier/value"),
        Some(&json!(1.25))
    );
    assert_eq!(
        material.pointer("/context/compositionInclusion/value"),
        Some(&json!({"type":"gameText", "sourceText":"Include"}))
    );
    let modifier = material
        .pointer("/context/modifiers/0")
        .ok_or("original property modifier")?;
    assert_eq!(
        modifier.pointer("/propertyGuid/value"),
        Some(&json!("00000000-0000-0000-0000-000011777336"))
    );
    let range = modifier
        .pointer("/ranges/0")
        .ok_or("the original linear range")?;
    assert_eq!(range.get("kind"), Some(&json!("multiplier")));
    assert_eq!(range.pointer("/startQuality/value"), Some(&json!("0")));
    assert_eq!(range.pointer("/endQuality/value"), Some(&json!("1000")));
    assert_eq!(range.pointer("/atStart/value"), Some(&json!(0.75)));
    assert_eq!(range.pointer("/atEnd/value"), Some(&json!(1.25)));
    assert_eq!(
        range.pointer("/atEnd/containsEncryptedSource"),
        Some(&json!(true))
    );
    Ok(())
}

fn rows(table: &Value) -> Result<&[Value], &'static str> {
    table
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or("reduced Blueprint rows")
}

fn with_queries(
    database: &DataForgeBuilder,
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted());
    let languages = [Language::English, Language::German];
    for (language, text) in languages
        .into_iter()
        .zip(["Fixture Blueprint", "Blaupause"])
    {
        let ini = GlobalIniBuilder::new()
            .text(fixture::LOCALE_KEY, text)
            .build();
        archive = archive.entry(TestEntry::stored(language.global_ini_path(), ini));
    }
    archive.write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&languages),
        None,
    )?;
    inspect(&session.parsed(None)?, scratch.path())
}
