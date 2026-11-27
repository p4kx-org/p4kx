// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ProvenanceMode,
};
use p4kx_export::{build_catalog, catalog_marker_schema, catalog_schema};
use p4kx_extraction::Language;
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as fixture, TestResult, at};
use crate::health_origin_assertions as proof;

const TRANSLATIONS: [(Language, &str, Option<bool>, &str); 3] = [
    (Language::English, "Shared text", Some(true), "encrypted"),
    (
        Language::German,
        "Gemeinsamer Text",
        Some(false),
        "unencrypted",
    ),
    (Language::French, "Texte partage", None, "unknown"),
];

#[test]
fn catalog_and_sdk_consumer_preserve_local_encryption_and_game_text_protection() -> TestResult {
    let db = fixture::with_keys("@shared", "@shared", "@resource_name");
    let french = TestEntry::stored(Language::French.global_ini_path(), b"shared=Texte partage")
        .encrypted()
        .with_encryption_field(7);
    let (_folder, archive) =
        fixture::open_bytes_with_entries(db.build(), fixture::DcbOrigin::Plain, [french])?;
    let options = fixture::options();
    let compact: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let full: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    assert_eq!(
        at(&compact, "/format")?,
        &json!("p4kx-catalog-markers"),
        "the ordinary Catalog uses the compact profile"
    );
    let compact = at(&compact, "/catalog")?;
    assert!(compact.get("provenance").is_none(), "no compact inventory");
    let compact_text = protected_text(compact)?;
    let full_text = protected_text(&full)?;
    assert_translation_profiles(compact_text, full_text, &full)?;
    assert_protected_schema()?;
    assert_sdk_profiles(&archive, compact_text, full_text, &full)
}

fn protected_text(catalog: &Value) -> TestResult<&Value> {
    let row = fixture::presentation(catalog, fixture::ALPHA)?;
    let public = at(row, "/name/value")?;
    let protected = at(row, "/description/value")?;
    assert_ne!(
        public, protected,
        "equal keys retain their own text domains"
    );
    let protected_row = fixture::text_row(catalog, "gameTexts", protected)?;
    assert_eq!(
        at(protected_row, "/key/value")?,
        at(
            fixture::text_row(catalog, "publicNames", public)?,
            "/key/value"
        )?,
        "the two domains are exercised with the same key"
    );
    Ok(protected_row)
}

fn assert_translation_profiles(compact: &Value, full: &Value, inventory: &Value) -> TestResult {
    assert_eq!(
        field_marker(at(compact, "/translations")?)?,
        Some(true),
        "a mixed row aggregate differs from the clear and unknown fields"
    );
    for (language, text, expected, declaration) in TRANSLATIONS {
        let compact = translation(compact, language)?;
        let full = translation(full, language)?;
        for cell in [compact, full] {
            assert_eq!(
                field_marker(cell)?,
                expected,
                "local {} marker",
                language.code()
            );
            assert_eq!(at(cell, "/state")?, &json!("present"), "readable value");
            assert_eq!(at(cell, "/value")?, &json!(text), "exact locale text");
        }
        assert!(
            compact.get("sourceUsesRefs").is_none() && compact.get("sourceSetComplete").is_none(),
            "compact fields omit detailed proof"
        );
        assert_original_locale(inventory, full, language, declaration)?;
    }
    Ok(())
}

fn assert_original_locale(
    inventory: &Value,
    cell: &Value,
    language: Language,
    declaration: &str,
) -> TestResult {
    crate::text_forms_assertions::direct(cell)?;
    let uses = proof::uses(inventory, cell)?;
    let mut localized_payloads = 0;
    for used in &uses {
        let source = proof::source(inventory, used)?;
        let path = at(source, "/path/value")?
            .as_str()
            .ok_or("original Entry path")?;
        if Language::from_global_ini_path(path) == Some(language) {
            assert_eq!(
                at(source, "/storedEncryption")?,
                &json!(declaration),
                "the exact locale source retains its stored declaration"
            );
            if used.get("role").and_then(Value::as_str) == Some("payload") {
                assert_lookup_selector(&uses, used, language)?;
                localized_payloads += 1;
            }
        } else {
            assert_eq!(
                path, "Data\\Game2.dcb",
                "only the actual authored key joins the locale"
            );
        }
    }
    assert!(
        localized_payloads > 0,
        "each field refers to the actual locale payload"
    );
    Ok(())
}

fn assert_lookup_selector(field_uses: &[&Value], lookup: &Value, language: Language) -> TestResult {
    assert_eq!(
        at(lookup, "/location/type")?,
        &json!("localizationLookup"),
        "a locale observation carries its actual lookup location"
    );
    assert_eq!(
        at(lookup, "/location/language")?,
        &json!(language.code()),
        "the lookup location names the actual language"
    );
    let selector = at(lookup, "/location/selector")?;
    assert!(
        field_uses
            .iter()
            .any(|used| used.get("id") == Some(selector)
                && used.get("role").and_then(Value::as_str) == Some("selector")),
        "the immediate field references its authored key selector"
    );
    Ok(())
}

fn assert_protected_schema() -> TestResult {
    for schema in [catalog_marker_schema(), catalog_schema()] {
        for marker in [
            "/$defs/GameTextRef/x-gameText",
            "/$defs/CommodityPresentation/properties/description/x-gameText",
            "/$defs/CatalogTexts/properties/gameTexts/x-gameText",
        ] {
            assert_eq!(
                at(&schema, marker)?,
                &json!(true),
                "the German description is protected even with a false encryption marker"
            );
        }
    }
    Ok(())
}

fn assert_sdk_profiles(
    archive: &p4kx_extraction::Archive,
    compact: &Value,
    full: &Value,
    full_catalog: &Value,
) -> TestResult {
    let selection = CatalogRecordSelection::Guid(fixture::guid(fixture::ALPHA).parse()?);
    for mode in [ProvenanceMode::MarkersOnly, ProvenanceMode::Full] {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::AllAvailable, None)?;
        let (expected, session) = if mode == ProvenanceMode::Full {
            (full, session.with_provenance_mode(mode))
        } else {
            (compact, session)
        };
        session.with_commodities(selection, None, |snapshot| -> TestResult {
            let mut matching = snapshot
                .game_texts
                .iter()
                .filter(|row| row.key().value().is_some_and(|key| *key == "@shared"));
            let actual = serde_json::to_value(matching.next().ok_or("scoped protected text")?)?;
            assert!(matching.next().is_none(), "one scoped protected row");
            assert_sdk_translation_profiles(
                &actual,
                expected,
                full_catalog,
                &snapshot.source_inventory,
                mode,
            )
        })??;
    }
    Ok(())
}

fn assert_sdk_translation_profiles(
    actual: &Value,
    expected: &Value,
    full_catalog: &Value,
    inventory: &p4kx_extraction::SourceInventory,
    mode: ProvenanceMode,
) -> TestResult {
    assert_eq!(
        inventory.provenance_mode(),
        mode,
        "the actual query provenance mode"
    );
    if mode == ProvenanceMode::Full {
        let sdk = json!({"provenance": inventory});
        for (language, _, _, _) in TRANSLATIONS {
            assert_eq!(
                proof::with_origins(translation(actual, language)?.clone(), &sdk)?,
                proof::with_origins(translation(expected, language)?.clone(), full_catalog)?,
                "SDK and catalog resolve the same physical {} references",
                language.code()
            );
        }
    } else {
        assert_eq!(
            inventory.iter_sources().len(),
            0,
            "default SDK physical inventory"
        );
        assert_eq!(inventory.iter_uses().len(), 0, "default SDK source uses");
        for (language, _, _, _) in TRANSLATIONS {
            assert_eq!(
                translation(actual, language)?,
                translation(expected, language)?,
                "SDK and catalog preserve the same immediate {} cell",
                language.code()
            );
        }
    }
    Ok(())
}

fn translation(row: &Value, language: Language) -> TestResult<&Value> {
    at(
        row,
        &format!("/translations/value/{}/displayText", language.code()),
    )
}

fn field_marker(cell: &Value) -> TestResult<Option<bool>> {
    match at(cell, "/containsEncryptedSource")? {
        Value::Bool(value) => Ok(Some(*value)),
        Value::Null => Ok(None),
        _ => Err("invalid field encryption marker".into()),
    }
}

#[test]
fn consumer_requires_the_own_marker_even_with_unrelated_metadata() {
    let missing = json!({
        "state": "present",
        "value": "input",
        "archiveKey": "example",
        "unreadCount": 0,
        "provenanceRef": "example",
        "row": {"containsEncryptedSource": false},
    });
    assert!(
        field_marker(&missing).is_err(),
        "no row or Archive fallback"
    );
}
