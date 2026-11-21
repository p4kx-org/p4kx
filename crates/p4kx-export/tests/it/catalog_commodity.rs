// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_export::Kind;
use serde_json::{Value, json};

use crate::catalog_crafting_build;
use crate::catalog_crafting_support::commodity::{self, TextDomain};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn commodities_are_selected_by_prefix_and_type_without_leftovers_ordered_by_guid() -> TestResult {
    let (built, catalog) = catalog_crafting_build::build(None)?;
    assert_eq!(
        commodity::class_names(&catalog)?,
        ["gold", "dopple", "ember", "iron"],
        "under entities/commodities, without the test commodity and the carryable, then the used ResourceType no commodity Record carries"
    );
    let kind = built.summary().kind(Kind::Commodities);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((4, 1, true)),
        "counts"
    );
    Ok(())
}

#[test]
fn a_commodity_carries_its_guids_texts_and_display_type() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let definition = &commodity::definitions(&catalog)?[0];
    let presentation = &commodity::presentations(&catalog)?[0];
    assert_eq!(
        (
            &definition["guid"]["value"],
            &definition["className"]["value"],
            &definition["recordPath"]["value"]
        ),
        (
            &json!("a1000000-0000-4000-8000-000000000001"),
            &json!("gold"),
            &json!("libs/foundry/records/entities/commodities/metals/gold")
        )
    );
    assert_eq!(definition["selection"]["value"], "entity");
    assert_eq!(
        presentation["commodityGuid"]["value"],
        definition["guid"]["value"]
    );
    assert_eq!(
        (
            &presentation["typeGuid"]["value"],
            &presentation["subtypeGuid"]["value"],
            &presentation["densityGramsPerCubicCentimetre"]["value"],
            &presentation["refinedSubtypeGuid"]["value"]
        ),
        (
            &json!("c1000000-0000-4000-8000-000000000001"),
            &json!("b1000000-0000-4000-8000-000000000001"),
            &json!(19.25),
            &Value::Null
        )
    );
    commodity::assert_text(
        &catalog,
        &presentation["name"],
        TextDomain::PublicName,
        Some("@items_commodities_gold"),
        Some("Gold"),
        Some("Gold"),
    )?;
    commodity::assert_text(
        &catalog,
        &presentation["description"],
        TextDomain::GameText,
        Some("@items_commodities_gold_desc"),
        Some("A precious metal."),
        Some("Ein Edelmetall."),
    )?;
    commodity::assert_text(
        &catalog,
        &presentation["displayType"],
        TextDomain::GameText,
        Some("@items_commodities_type_metal"),
        Some("Metal"),
        Some("Metall"),
    )?;
    Ok(())
}

#[test]
fn a_commodity_keeps_its_own_description_and_may_lack_a_resource_type() -> TestResult {
    let (_, catalog) = catalog_crafting_build::build(None)?;
    let presentation = &commodity::presentations(&catalog)?[1];
    commodity::assert_text(
        &catalog,
        &presentation["description"],
        TextDomain::GameText,
        Some("@items_commodities_dopple_desc"),
        Some("A vice."),
        None,
    )?;
    assert_eq!(presentation["typeGuid"]["value"], Value::Null, "no type");
    Ok(())
}
