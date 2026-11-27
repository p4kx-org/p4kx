// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use serde_json::{Value, json};

use super::super::plan;
use super::super::selected_tables::{Selection, is_staged_dataset};
use super::basetool_catalog_schema;
use crate::contract::SCHEMA_VERSION;
use crate::ship_store_pledge::ship_store_pledges;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn accepts_the_reduced_envelope_and_typed_localization() -> TestResult {
    let schema = basetool_catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    let document = fixture()?;
    assert!(
        validator.is_valid(&document),
        "the reduced document satisfies its structural schema"
    );
    assert_eq!(
        schema.pointer("/properties/catalog/properties/texts/properties/gameTexts/items/properties/id/properties/value/x-gameText"),
        Some(&json!(true)),
        "the protected text identifier keeps its schema classification"
    );
    Ok(())
}

#[test]
fn rejects_wrong_profile_metadata_scope_and_unavailable_tables() -> TestResult {
    let schema = basetool_catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    let document = fixture()?;
    let loadouts_path = coverage_selection("loadouts")?;
    for (pointer, invalid) in [
        ("/format", json!("p4kx-catalog-markers")),
        ("/formatVersion", json!(2)),
        ("/catalog/schemaVersion", json!(2)),
        ("/catalog/source/build/changeNumber", json!(123)),
        ("/catalog/exportHealth/status", json!("partial")),
        ("/catalog/exportHealth/scope", json!(["items"])),
        ("/catalog/configurations", json!({})),
        ("/catalog/projections/jumpConnections", table("unavailable")),
        (
            "/catalog/projections/jumpConnections",
            json!({"availability":"absent","rows":[{}]}),
        ),
        ("/catalog/shops", json!({})),
        (
            "/catalog/texts/publicNames/0/translations/value",
            json!({"fr":forms("Absent locale")}),
        ),
        (
            "/catalog/texts/publicNames/0/translations/value/en/sourceText/value",
            Value::Null,
        ),
        (&loadouts_path, json!("all")),
    ] {
        let mut malformed = document.clone();
        *malformed
            .pointer_mut(pointer)
            .ok_or("fixture schema mutation path")? = invalid;
        assert!(
            !validator.is_valid(&malformed),
            "the reduced schema must reject an invalid {pointer}"
        );
    }
    let mut malformed = document;
    let Some(catalog) = malformed.get_mut("catalog").and_then(Value::as_object_mut) else {
        return Err("fixture catalog object".into());
    };
    catalog.insert("provenance".to_owned(), json!({}));
    assert!(
        !validator.is_valid(&malformed),
        "a generic provenance member cannot enter this publication profile"
    );
    Ok(())
}

fn coverage_selection(path: &str) -> TestResult<String> {
    let position = plan::SELECTION
        .into_iter()
        .filter(|path| is_staged_dataset(path))
        .position(|selected| selected == path)
        .ok_or("dataset coverage position")?;
    Ok(format!("/catalog/completeness/{position}/selection"))
}

fn fixture() -> TestResult<Value> {
    let mut catalog = fixture_metadata()?;
    let content = catalog.as_object_mut().ok_or("fixture catalog members")?;
    let mut coverage = Vec::new();
    for path in plan::SELECTION
        .into_iter()
        .filter(|path| is_staged_dataset(path))
    {
        if let Some((section, dataset)) = path.split_once('.') {
            let section = content.entry(section).or_insert_with(|| json!({}));
            let items = section.as_object_mut().ok_or("fixture catalog section")?;
            items.insert(dataset.to_owned(), table("available"));
        } else {
            content.insert(path.to_owned(), json!([]));
        }
        coverage.push(json!({"path": path, "rows": 0, "selection": Selection::for_path(path)}));
    }
    content.insert("completeness".to_owned(), json!(coverage));
    Ok(json!({"format": plan::FORMAT, "formatVersion": plan::FORMAT_VERSION, "catalog": catalog}))
}

fn fixture_metadata() -> TestResult<Value> {
    Ok(json!({
        "schemaVersion": SCHEMA_VERSION,
        "source": {
            "channel": "LIVE",
            "build": {
                "branch": "sc-alpha-5.0",
                "version": "5.0.0",
                "changeNumber": "123",
                "buildId": "fixture",
                "publicPatch": null,
            },
            "archiveSize": 1,
            "archiveModified": null,
            "archiveKey": "given",
            "encryptedEntriesUnread": 0,
            "toolVersion": "0.5.0",
            "exportedAt": "2026-11-27T00:00:00Z",
        },
        "exportHealth": {"status": "complete", "scope": plan::SELECTION.as_slice()},
        "shipStorePledges": ship_store_pledges()?,
        "texts": {
            "publicNames": [{
                "id": present(&json!("name:8:@fixture")),
                "key": present(&json!("@fixture")),
                "translations": present(&json!({"en": forms("Name"), "de": forms("Bezeichnung")})),
            }],
            "gameTexts": [{
                "id": present(&json!("text:8:@fixture")),
                "key": present(&json!("@fixture")),
                "translations": present(&json!({
                    "en": {
                        "sourceText":{"state":"absent","value": null,"containsEncryptedSource":null},
                        "displayText":{"state":"absent","value": null,"containsEncryptedSource":null}
                    }
                })),
            }],
        },
    }))
}

fn table(availability: &str) -> Value {
    json!({"availability": availability, "rows": []})
}

fn forms(text: &str) -> Value {
    json!({"sourceText": present(&json!(text)), "displayText": present(&json!(text))})
}

fn present(value: &Value) -> Value {
    json!({"state": "present", "value": value, "containsEncryptedSource": true})
}
