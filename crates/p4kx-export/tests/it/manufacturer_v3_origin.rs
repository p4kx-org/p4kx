// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::Language;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::DcbOrigin;
use crate::commodity_v3_source_inventory::{self, SourceField, body, header};
use crate::manufacturer_v3_fixture::{self as fixture, TestResult};

type DirectField = (&'static str, SourceField, &'static str);

fn direct_fields() -> [DirectField; 12] {
    [
        ("/guid", header("guid"), "payload"),
        ("/recordName", header("recordName"), "payload"),
        ("/recordPath", header("fileName"), "conversion"),
        ("/recordIndex", header("recordIndex"), "selector"),
        ("/sourceType", header("sourceType"), "selector"),
        ("/fileName", header("fileName"), "payload"),
        ("/metadataTag", header("metadataTag"), "payload"),
        ("/code", body("Code"), "payload"),
        ("/localization", body("Localization"), "payload"),
        (
            "/localization/value/sourceType",
            body("Localization.$type"),
            "selector",
        ),
        (
            "/localization/value/name",
            body("Localization.Name"),
            "payload",
        ),
        (
            "/localization/value/description",
            body("Localization.Description"),
            "payload",
        ),
    ]
}

#[test]
fn actual_payload_selector_and_conversion_uses_stay_at_their_physical_fields() -> TestResult {
    let (_, catalog) = fixture::normal(&fixture::database())?;
    let row = fixture::row(&catalog)?;
    for (carrier, field, role) in direct_fields() {
        let value = fixture::at(row, carrier)?;
        assert_eq!(
            commodity_v3_source_inventory::uses(&catalog, value)?,
            BTreeSet::from([(0, field, role.to_owned())]),
            "only the actual field contributes to {carrier}"
        );
        assert_eq!(
            fixture::at(value, "/containsEncryptedSource")?,
            &json!(false)
        );
    }
    assert_eq!(
        commodity_v3_source_inventory::uses(&catalog, fixture::at(row, "/className")?)?,
        BTreeSet::from([
            (0, header("recordName"), "conversion".to_owned()),
            (0, header("sourceType"), "selector".to_owned())
        ])
    );
    commodity_v3_source_inventory::reference_closure(&catalog)?;
    Ok(())
}

#[test]
fn encrypted_localization_does_not_taint_headers_code_or_authored_text_refs() -> TestResult {
    let (_, catalog) = fixture::normal(&fixture::database())?;
    let name = fixture::text(&catalog, "publicNames", "name")?;
    for (language, expected) in [("en", true), ("de", false)] {
        assert_markers(name, language, &json!(expected))?;
    }
    for path in [
        "/guid",
        "/code",
        "/localization",
        "/localization/value/name",
        "/localization/value/description",
    ] {
        assert_eq!(
            fixture::at(
                fixture::at(fixture::row(&catalog)?, path)?,
                "/containsEncryptedSource"
            )?,
            &json!(false),
            "only actual inputs mark {path}"
        );
    }
    Ok(())
}

#[test]
fn original_encrypted_and_unknown_dcb_origins_reach_every_direct_fact() -> TestResult {
    for (origin, expected) in [
        (DcbOrigin::Encrypted, json!(true)),
        (DcbOrigin::Unknown, Value::Null),
    ] {
        let (_, catalog) = fixture::build(
            &fixture::database(),
            origin,
            CatalogPublicationMode::DiagnosticPartial,
            None,
        )?;
        let row = fixture::row(&catalog)?;
        for field in [
            "guid",
            "recordName",
            "className",
            "recordPath",
            "recordIndex",
            "sourceType",
            "code",
            "localization",
        ] {
            assert_eq!(
                fixture::at(
                    fixture::at(row, &format!("/{field}"))?,
                    "/containsEncryptedSource"
                )?,
                &expected
            );
        }
        assert_markers(
            fixture::text(&catalog, "publicNames", "name")?,
            "de",
            &expected,
        )?;
        commodity_v3_source_inventory::reference_closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn every_available_language_retains_its_own_winning_payload_and_crypto_index() -> TestResult {
    let database = fixture::with_keys("@fixture_name", "@fixture_description");
    let (_directory, archive) = crate::resource_origin_fixture::open(database.build(), true)?;
    let mut options = crate::resource_origin_fixture::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let name = fixture::text(&catalog, "publicNames", "name")?;
    assert_winning_forms(name)?;
    assert_eq!(
        fixture::at(fixture::row(&catalog)?, "/code/containsEncryptedSource")?,
        &json!(false)
    );
    commodity_v3_source_inventory::reference_closure(&catalog)?;
    Ok(())
}

fn assert_winning_forms(name: &Value) -> TestResult {
    let translations = fixture::at(name, "/translations/value")?
        .as_object()
        .ok_or("language map")?;
    assert_eq!(
        translations.len(),
        Language::ALL.len(),
        "no available supported language is dropped"
    );
    for language in Language::ALL {
        let translation = translations
            .get(language.code())
            .ok_or("available language carrier")?;
        for field in ["sourceText", "displayText"] {
            let value = fixture::at(translation, &format!("/{field}"))?;
            crate::text_forms_assertions::direct(value)?;
            assert_eq!(
                fixture::at(value, "/state")?,
                &json!("present"),
                "every available language retains both winning text forms"
            );
        }
    }
    for field in ["sourceText", "displayText"] {
        assert_eq!(
            fixture::at(name, &format!("/translations/value/en/{field}/value"))?,
            &json!("English winner"),
            "the original English winner is unchanged"
        );
        assert_eq!(
            fixture::at(name, &format!("/translations/value/de/{field}/value"))?,
            &json!("Translated name"),
            "the actual German winner is retained"
        );
        assert_eq!(
            fixture::at(
                name,
                &format!("/translations/value/fr/{field}/containsEncryptedSource")
            )?,
            &Value::Null,
            "unknown encryption remains explicit per form"
        );
    }
    Ok(())
}

fn assert_markers(name: &Value, language: &str, expected: &Value) -> TestResult {
    let forms = crate::text_forms_assertions::forms(name, language)?;
    for field in ["sourceText", "displayText"] {
        let value = forms.get(field).ok_or("actual direct text form")?;
        crate::text_forms_assertions::direct(value)?;
        assert_eq!(
            fixture::at(value, "/containsEncryptedSource")?,
            expected,
            "both direct forms retain the original language/key encryption inputs"
        );
    }
    Ok(())
}
