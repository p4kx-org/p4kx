// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::Language;
use serde_json::{Value, json};

#[path = "title_origin_fixture.rs"]
mod fixture;

use self::fixture::{Original, Site};
use crate::health_origin_assertions as assertions;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};
use crate::text_forms_origin_assertions as text_origins;

#[test]
fn contract_titles_keep_actual_language_and_layer_contributions() -> TestResult {
    let cases = [Input::Plain, Input::Encrypted, Input::Standalone]
        .into_iter()
        .map(|input| fixture::open(input).map(|(fixture, original)| (input, fixture, original)))
        .collect::<TestResult<Vec<_>>>()?;
    for (input, fixture, original) in cases {
        let catalog = build(&fixture)?;
        let parent = title_row(&catalog, &original, None)?;
        let child = title_row(&catalog, &original, Some(original.child))?;
        check_language_cells(
            &catalog,
            parent,
            &original,
            input,
            "Inspect Parent ready",
            original.parent_piece,
        )?;
        check_language_cells(
            &catalog,
            child,
            &original,
            input,
            "Inspect Child ready",
            original.child_piece,
        )?;
        let child_key = assertions::at(child, "/title/key")?;
        assert!(
            assertions::uses(&catalog, child_key)?.iter().any(|used| {
                same_site(used, original.rejected)
                    && used.get("role").and_then(Value::as_str) == Some("selector")
            }),
            "the actually interpreted first child override explains the fallback"
        );
        assert!(
            catalog.get("contracts").is_none(),
            "canonical V contracts have one wire location"
        );
    }
    Ok(())
}

#[test]
fn description_fallback_uses_the_actual_broker_description_site() -> TestResult {
    let (fixture, original) = fixture::open(Input::Plain)?;
    let catalog = build(&fixture)?;
    let row = title_row(&catalog, &original, Some(original.child))?;
    let description = assertions::at(row, "/description")?;
    key(
        &catalog,
        description,
        fixture::DESCRIPTION,
        original.description,
    )?;
    let english = translation(description, Language::English)?;
    let german = translation(description, Language::German)?;
    assert_eq!(
        assertions::at(english, "/value")?,
        "Verify\nData",
        "the actual description normalization"
    );
    assert_eq!(
        assertions::at(german, "/value")?,
        fixture::GERMAN_DESCRIPTION,
        "German description preserves its localized value"
    );
    origin(english, Some(true))?;
    origin(german, Some(false))?;
    lookup_payloads(
        &catalog,
        english,
        Language::English,
        &[original.description],
    )?;
    lookup_payloads(&catalog, german, Language::German, &[original.description])?;
    for cell in [english, german] {
        no_sites(
            &catalog,
            cell,
            &[
                original.title,
                original.template_title,
                original.broker_title,
            ],
        )?;
    }
    Ok(())
}

fn build(fixture: &Fixture) -> TestResult<Value> {
    let built = p4kx_export::build_catalog_from(
        &fixture.archive,
        &fixture.entry,
        &health_origin_fixture::options(),
    )?;
    assert!(
        built.summary().is_complete(),
        "all controlled source data is actually readable"
    );
    Ok(serde_json::from_slice(built.json())?)
}

fn title_row<'document>(
    catalog: &'document Value,
    original: &Original,
    child: Option<Site>,
) -> TestResult<&'document Value> {
    let definition = assertions::rows(catalog, "/definitions/contractGenerators/rows")?
        .iter()
        .find(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64)
                == Some(u64::from(original.contract.record))
        })
        .ok_or("actual physical Generator definition")?;
    for (site, source_type) in [
        (original.handler, "ContractGeneratorHandler_List"),
        (original.contract, "CareerContract"),
        (original.child, "SubContract"),
    ] {
        let values = assertions::rows(definition, "/source/values")?;
        let value = values
            .get(usize::try_from(site.ordinal)?)
            .ok_or("actual original source value")?;
        assert_eq!(
            assertions::at(value, "/value/value/sourceType/value")?,
            source_type,
            "the physical source graph preserves each original occurrence"
        );
    }
    let contract_guid = health_origin_fixture::guid(fixture::CONTRACT);
    let child_guid = child.map(|_| health_origin_fixture::guid(fixture::CHILD));
    let contract = assertions::rows(catalog, "/projections/contracts/rows")?
        .iter()
        .find(|row| {
            row.pointer("/guid/value").and_then(Value::as_str) == Some(contract_guid.as_str())
                && row
                    .pointer("/subContractGuid/value")
                    .and_then(Value::as_str)
                    == child_guid.as_deref()
        })
        .ok_or("canonical contract or sub-contract projection")?;
    let expected_source = json!({
        "inputRef": assertions::at(definition, "/inputRef")?,
        "recordIndex": original.contract.record,
        "handler": value_ref(original.handler),
        "contract": value_ref(original.contract),
        "subContract": child.map(value_ref),
    });
    assert_eq!(
        assertions::at(contract, "/source")?,
        &expected_source,
        "the logical projection links to its actual owning original occurrences"
    );
    assertions::rows(catalog, "/projections/missionTitles/rows")?
        .iter()
        .find(|row| row.get("contract") == Some(&expected_source))
        .ok_or_else(|| {
            "actual mission title row joins the same physical contract occurrence".into()
        })
}

fn value_ref(site: Site) -> Value {
    json!({"valueIndex": site.ordinal.to_string()})
}

fn check_language_cells(
    catalog: &Value,
    row: &Value,
    original: &Original,
    input: Input,
    expected_english: &str,
    chosen: Site,
) -> TestResult {
    let title = assertions::at(row, "/title")?;
    key(catalog, title, fixture::TITLE, original.title)?;
    let english = translation(title, Language::English)?;
    let german = translation(title, Language::German)?;
    assert_eq!(
        assertions::at(english, "/value")?,
        expected_english,
        "the case-folded nearest token and nested resolution both win"
    );
    assert_eq!(
        assertions::at(german, "/value")?,
        fixture::GERMAN_TITLE,
        "the actual German source is literal"
    );
    origin(english, Some(true))?;
    origin(german, input.marker())?;
    lookup_payloads(
        catalog,
        english,
        Language::English,
        &[original.title, chosen, original.nested],
    )?;
    lookup_payloads(catalog, german, Language::German, &[original.title])?;
    for cell in [english, german] {
        no_sites(
            catalog,
            cell,
            &[
                original.description,
                original.later,
                original.template_title,
                original.broker_title,
            ],
        )?;
    }
    no_sites(
        catalog,
        german,
        &[original.parent_piece, original.child_piece, original.nested],
    )?;
    let other = if chosen.ordinal == original.parent_piece.ordinal {
        original.child_piece
    } else {
        original.parent_piece
    };
    no_sites(catalog, english, &[other])?;
    missing_french_lookup(catalog, title, original.title)
}

fn missing_french_lookup(catalog: &Value, title: &Value, title_site: Site) -> TestResult {
    let french = translation(title, Language::French)?;
    assert_eq!(
        assertions::at(french, "/state")?,
        "absent",
        "available language and absent key are distinct"
    );
    lookup_payloads(catalog, french, Language::French, &[])?;
    let inventory = assertions::rows(catalog, "/provenance/uses")?;
    let mut missing_key = false;
    for used in assertions::uses(catalog, french)? {
        if assertions::at(used, "/location/type")? == "localizationLookup"
            && used.get("role").and_then(Value::as_str) == Some("selector")
        {
            let selector =
                text_origins::unique(inventory, assertions::at(used, "/location/selector")?)?;
            missing_key |= same_site(selector, title_site);
        }
    }
    assert!(
        missing_key,
        "the readable French Entry was queried with the actual title key"
    );
    Ok(())
}

fn key(catalog: &Value, text: &Value, expected: &str, site: Site) -> TestResult {
    let key = assertions::at(text, "/key")?;
    let id = assertions::at(key, "/value")?;
    let row = assertions::rows(catalog, "/texts/gameTexts")?
        .iter()
        .find(|row| row.pointer("/id/value") == Some(id))
        .ok_or("actual protected text row")?;
    assert_eq!(
        assertions::at(row, "/key/value")?,
        expected,
        "the original key is stored in the protected domain"
    );
    assert!(
        assertions::uses(catalog, key)?
            .iter()
            .any(|used| same_site(used, site)),
        "the key reference has its own exact authored occurrence"
    );
    Ok(())
}

fn translation(text: &Value, language: Language) -> TestResult<&Value> {
    assertions::at(text, &format!("/translations/{}", language.code()))
}

fn origin(cell: &Value, expected: Option<bool>) -> TestResult {
    assert_eq!(
        assertions::at(cell, "/state")?,
        "present",
        "the selected localized cell contains a value"
    );
    assert_eq!(
        assertions::at(cell, "/containsEncryptedSource")?,
        &json!(expected),
        "the cell reports the encryption of the selected inputs"
    );
    assert_eq!(
        assertions::at(cell, "/sourceSetComplete")?,
        true,
        "known physical scope and unknown original encryption remain separate"
    );
    Ok(())
}

fn same_site(used: &Value, site: Site) -> bool {
    used.pointer("/location/type").and_then(Value::as_str) == Some("subresource")
        && used.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeValue")
        && used.pointer("/location/indices")
            == Some(&json!([site.record.to_string(), site.ordinal.to_string()]))
        && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
}

fn no_sites(catalog: &Value, cell: &Value, excluded: &[Site]) -> TestResult {
    for used in assertions::uses(catalog, cell)? {
        assert!(
            excluded.iter().all(|site| !same_site(used, *site)),
            "an unused authored key does not contribute to this result: {used}"
        );
    }
    Ok(())
}

fn lookup_payloads(
    catalog: &Value,
    cell: &Value,
    language: Language,
    expected: &[Site],
) -> TestResult {
    let inventory = assertions::rows(catalog, "/provenance/uses")?;
    let uses = assertions::uses(catalog, cell)?;
    let mut payloads = BTreeSet::new();
    for used in uses {
        let source = assertions::source(catalog, used)?;
        if assertions::at(used, "/location/type")? != "localizationLookup" {
            if assertions::at(source, "/kind")? == "archiveEntry" {
                assert_eq!(
                    assertions::at(source, "/path/value")?,
                    "Data\\Game2.dcb",
                    "an unrelated physical locale cannot masquerade as a non-lookup use"
                );
            }
            continue;
        }
        assert_eq!(
            assertions::at(used, "/location/language")?,
            language.code(),
            "another language cannot contribute to this rendered cell"
        );
        assert_eq!(
            assertions::at(source, "/kind")?,
            "archiveEntry",
            "each localization lookup uses an original archive Entry"
        );
        assert_eq!(
            assertions::at(source, "/path/value")?,
            language.global_ini_path(),
            "the actual lookup uses the original language Entry"
        );
        let selector =
            text_origins::unique(inventory, assertions::at(used, "/location/selector")?)?;
        assert_eq!(
            assertions::at(selector, "/role")?,
            "selector",
            "the locale lookup identifies its actual key selector"
        );
        if assertions::at(used, "/role")? == "payload" {
            let site = expected
                .iter()
                .find(|site| same_site(selector, **site))
                .ok_or("locale payload uses an unrelated original key selector")?;
            payloads.insert((site.record, site.ordinal));
        }
    }
    let expected: BTreeSet<_> = expected
        .iter()
        .map(|site| (site.record, site.ordinal))
        .collect();
    assert_eq!(
        payloads, expected,
        "all and only actual localized payloads contribute"
    );
    Ok(())
}
