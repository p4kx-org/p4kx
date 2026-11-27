// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Language;
use p4kx_testkit::TestRecord;
use serde_json::Value;

use super::commodities;
use super::resource_type_tests::open_fixture;
use super::selected_texts::SelectedTexts;
use crate::build::api::{OverrideEntryTarget, resource_fixture};
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn reduced_commodities_keep_physical_pairs_shared_resources_and_protected_texts() -> TestResult {
    let database = resource_fixture::database(OverrideEntryTarget::Base).record(TestRecord::new(
        resource_fixture::COMMODITY_ALPHA,
        "EntityClassDefinition",
        "EntityClassDefinition.FixtureAlphaAlias",
        "libs/foundry/records/entities/commodities/fixture_alpha_alias.xml",
    ));
    let (scratch, archive) = open_fixture(&database)?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    let mut texts = SelectedTexts::default();
    let tables = commodities::collect(&session.parsed(None)?, scratch.path(), None, &mut texts)?;
    let definitions = serde_json::to_value(tables.definitions)?;
    let presentations = serde_json::to_value(tables.presentations)?;
    let definitions = definitions
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("definition rows")?;
    let presentations = presentations
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("presentation rows")?;
    let original = assert_physical_pairs(definitions, presentations)?;
    assert_protected_presentation(presentations, original, &serde_json::to_value(texts)?)
}

fn assert_physical_pairs(definitions: &[Value], presentations: &[Value]) -> TestResult<u64> {
    assert_eq!(
        definitions.len(),
        presentations.len(),
        "same physical population"
    );

    let mut alias_records = Vec::new();
    let mut linked = Vec::new();
    let mut unlinked = None;
    for (definition, presentation) in definitions.iter().zip(presentations) {
        let index = assert_row_pair(definition, presentation)?;
        let class_name = definition
            .pointer("/className/value")
            .and_then(Value::as_str);
        if matches!(class_name, Some("FixtureAlpha" | "FixtureAlphaAlias")) {
            alias_records.push(index);
        }
        if matches!(class_name, Some("FixtureAlpha" | "FixtureBeta")) {
            let resource = presentation
                .pointer("/subtypeGuid/value")
                .and_then(Value::as_str)
                .ok_or("the original Commodity has a resource")?;
            linked.push((resource, index));
        }
        if class_name == Some("FixtureUnlinked") {
            unlinked = Some(presentation);
        }
    }
    let [original, alias] = alias_records.as_slice() else {
        return Err("two physical FixtureAlpha rows".into());
    };
    assert_ne!(
        original, alias,
        "both same-GUID owner positions were retained"
    );
    assert_commodity_links(&linked, unlinked)?;
    Ok(*original)
}

fn assert_row_pair(definition: &Value, presentation: &Value) -> TestResult<u64> {
    let index = definition
        .pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .ok_or("physical definition index")?;
    assert_eq!(
        definition.pointer("/guid/value"),
        presentation.pointer("/commodityGuid/value"),
        "definition and presentation retain their canonical GUID pair"
    );
    assert_eq!(
        presentation
            .pointer("/recordIndex/value")
            .and_then(Value::as_u64),
        Some(index),
        "the emitted index separates physical aliases of one GUID"
    );
    assert!(
        definition.get("entity").is_none(),
        "the identity-only definition excludes authoring payloads"
    );
    Ok(index)
}

fn assert_commodity_links(linked: &[(&str, u64)], unlinked: Option<&Value>) -> TestResult {
    let [
        (first_resource, first_index),
        (second_resource, second_index),
    ] = linked
    else {
        return Err("two Commodities that share one resource without coalescing".into());
    };
    assert_eq!(first_resource, second_resource);
    assert_ne!(first_index, second_index);
    let unlinked = unlinked.ok_or("unlinked commodity")?;
    assert_eq!(
        unlinked
            .pointer("/subtypeGuid/state")
            .and_then(Value::as_str),
        Some("present")
    );
    assert!(
        unlinked
            .pointer("/subtypeGuid/value")
            .is_some_and(Value::is_null)
    );

    Ok(())
}

fn assert_protected_presentation(
    presentations: &[Value],
    original: u64,
    texts: &Value,
) -> TestResult {
    let original = presentations
        .iter()
        .find(|row| row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(original))
        .ok_or("original presentation")?;
    assert_eq!(
        original
            .pointer("/densityGramsPerCubicCentimetre/value")
            .and_then(Value::as_f64),
        Some(3.25)
    );
    assert_eq!(
        original
            .pointer("/description/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true)
    );
    let description = original
        .pointer("/description/value")
        .and_then(Value::as_str)
        .ok_or("selected protected description reference")?;
    let game_text = texts
        .get("gameTexts")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.pointer("/id/value").and_then(Value::as_str) == Some(description))
        })
        .ok_or("protected text closure")?;
    assert_eq!(
        game_text
            .pointer("/translations/value/de/displayText/value")
            .and_then(Value::as_str),
        Some("Geschuetzter Text")
    );
    assert_eq!(
        game_text
            .pointer("/translations/value/en/displayText/value")
            .and_then(Value::as_str),
        Some("Protected")
    );
    Ok(())
}
