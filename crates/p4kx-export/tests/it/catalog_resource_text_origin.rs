// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Product-wire text states and source uses for canonical resource text rows.

use p4kx_export::{BuiltCatalog, build_catalog};
use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use crate::resource_origin_fixture::{self as fixture, TestResult};

pub(super) fn catalog(name: &str, description: &str, english: &str) -> TestResult<Value> {
    let built = built_catalog(name, description, english)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn built_catalog(name: &str, description: &str, english: &str) -> TestResult<BuiltCatalog> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::database_with_texts(1.0, name, description).build(),
        ))
        .entry(
            TestEntry::stored(Language::English.global_ini_path(), english.as_bytes()).encrypted(),
        )
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok(build_catalog(&archive, &fixture::options())?)
}

pub(super) fn text_row<'a>(catalog: &'a Value, domain: &str, key: &str) -> TestResult<&'a Value> {
    catalog
        .get("texts")
        .and_then(|texts| texts.get(domain))
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.pointer("/key/value").and_then(Value::as_str) == Some(key))
        })
        .ok_or_else(|| "canonical text row missing".into())
}

fn english<'a>(catalog: &'a Value, key: &str) -> TestResult<&'a Value> {
    text_row(catalog, "publicNames", key)?
        .pointer("/translations/value/en")
        .ok_or_else(|| "English text state missing".into())
}

#[test]
fn placeholders_do_not_consume_an_available_encrypted_locale() -> TestResult {
    let catalog = catalog(
        "@LOC_EMPTY",
        "@LOC_EMPTY",
        "LOC_EMPTY=ignored locale payload",
    )?;
    for domain in ["publicNames", "gameTexts"] {
        let row = text_row(&catalog, domain, "@LOC_EMPTY")?;
        let value = row
            .pointer("/translations/value/en")
            .ok_or("placeholder state")?;
        assert_eq!(value["state"], "absent", "placeholder retains no text");
        assert_eq!(
            value["containsEncryptedSource"], false,
            "only the plain DCB selector contributes"
        );
        assert_eq!(
            row["translations"]["containsEncryptedSource"], false,
            "aggregate excludes unused locale"
        );
    }
    let uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    assert!(
        uses.iter().all(|used| {
            used.pointer("/location/type").and_then(Value::as_str) != Some("localizationLookup")
        }),
        "placeholder lookup never happened"
    );
    Ok(())
}

#[test]
fn declared_empty_and_trimmed_empty_texts_are_present_and_missing_keys_are_absent() -> TestResult {
    for (text, state, displayed) in [
        ("fixture_name=", "present", Some("")),
        ("fixture_name=  \\n  ", "present", Some("")),
        ("other=unrelated", "absent", None),
    ] {
        let catalog = catalog("@fixture_name", "@LOC_EMPTY", text)?;
        let value = english(&catalog, "@fixture_name")?;
        assert_eq!(
            value["state"], state,
            "source presence is independent of display emptiness"
        );
        assert_eq!(
            value["value"].as_str(),
            displayed,
            "display conversion is retained"
        );
        assert_eq!(
            value["containsEncryptedSource"], true,
            "the actual locale was searched"
        );
        assert_eq!(
            value["sourceSetComplete"], true,
            "the lookup source set is complete"
        );
    }
    Ok(())
}

#[test]
fn suffix_case_and_duplicate_winner_semantics_are_preserved() -> TestResult {
    for (text, expected) in [
        (
            "fixture_name=\nFIXTURE_NAME,P=Suffix winner",
            "Suffix winner",
        ),
        (
            "fixture_name=discarded\nFIXTURE_NAME=Last winner\nfixture_name,P=Ignored suffix",
            "Last winner",
        ),
        ("fixture_name=   \nfixture_name,P=Ignored suffix", ""),
    ] {
        let catalog = catalog("@FiXtUrE_NaMe", "@LOC_EMPTY", text)?;
        let value = english(&catalog, "@FiXtUrE_NaMe")?;
        assert_eq!(value["state"], "present", "winning declaration is present");
        assert_eq!(
            value["value"], expected,
            "the existing winner and trimming rules apply"
        );
    }
    Ok(())
}

#[test]
fn bare_keys_resolve_but_literal_looking_inputs_never_escape_into_global_source_metadata()
-> TestResult {
    let sentinel = "PRIVATE literal without a locale key";
    let catalog = catalog("fixture_name", sentinel, "fixture_name=Bare key winner")?;
    assert_eq!(
        english(&catalog, "fixture_name")?["value"],
        "Bare key winner",
        "bare keys retain lookup behavior"
    );
    let private = text_row(&catalog, "gameTexts", sentinel)?;
    assert_eq!(
        private["key"]["value"], sentinel,
        "original Locale field stays in its protected domain"
    );
    assert_eq!(
        private["translations"]["value"]["en"]["state"], "absent",
        "no literal fallback is invented"
    );
    assert!(
        !serde_json::to_string(&catalog["provenance"])?.contains(sentinel),
        "global use IDs and source locations cannot expose the protected original string"
    );
    check_lookup_selectors(&catalog)?;
    Ok(())
}

#[test]
fn shared_keys_retain_each_actual_dcb_selector_and_the_exact_language_entry() -> TestResult {
    let catalog = catalog(
        "@fixture_name",
        "@fixture_name",
        "fixture_name=Shared winner",
    )?;
    check_lookup_selectors(&catalog)?;
    let uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let lookups: Vec<_> = uses
        .iter()
        .filter(|used| {
            used.pointer("/location/type").and_then(Value::as_str) == Some("localizationLookup")
        })
        .collect();
    assert!(!lookups.is_empty(), "actual lookups are registered");
    let selectors: std::collections::BTreeSet<_> = lookups
        .iter()
        .filter_map(|used| used.pointer("/location/selector").and_then(Value::as_str))
        .collect();
    assert_eq!(
        selectors.len(),
        2,
        "name and description keep their own DCB selection"
    );
    let sources = catalog
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    for used in lookups {
        let source = sources
            .iter()
            .find(|source| source.get("id") == used.get("source"))
            .ok_or("lookup source")?;
        assert_eq!(
            source["path"]["value"],
            Language::English.global_ini_path(),
            "lookup uses the actual language Entry"
        );
    }
    Ok(())
}

fn check_lookup_selectors(catalog: &Value) -> TestResult {
    let uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    for used in uses.iter().filter(|used| {
        used.pointer("/location/type").and_then(Value::as_str) == Some("localizationLookup")
    }) {
        let selector_id = used
            .pointer("/location/selector")
            .ok_or("lookup selector id is missing")?;
        let selector = uses
            .iter()
            .find(|selector| selector.get("id") == Some(selector_id))
            .ok_or("lookup selector does not resolve")?;
        assert_eq!(
            selector.pointer("/location/type").and_then(Value::as_str),
            Some("dataForge"),
            "selector is an actual DCB field"
        );
        assert_eq!(selector["role"], "selector", "source role is precise");
        assert!(
            matches!(
                selector.pointer("/location/field").and_then(Value::as_str),
                Some("displayName" | "description")
            ),
            "selector names its actual resource field"
        );
    }
    Ok(())
}

#[test]
fn text_summary_keeps_legacy_counts_without_inventing_missing_language_rows() -> TestResult {
    let built = built_catalog("@fixture_name", "@LOC_EMPTY", "fixture_name=Name")?;
    let summaries = built.summary().texts();
    let languages: Vec<_> = summaries.iter().map(|summary| summary.language).collect();
    assert_eq!(
        languages,
        [Language::English, Language::German],
        "legacy availability rows remain"
    );
    let english = summaries
        .iter()
        .find(|summary| summary.language == Language::English)
        .ok_or("English summary")?;
    let german = summaries
        .iter()
        .find(|summary| summary.language == Language::German)
        .ok_or("German summary")?;
    assert_eq!(
        (english.resolved, english.unresolved, english.placeholder),
        (1, 0, 1),
        "one nonempty key and one placeholder in English"
    );
    assert_eq!(
        (german.resolved, german.unresolved, german.placeholder),
        (0, 1, 1),
        "the missing German lookup is counted once"
    );
    Ok(())
}
