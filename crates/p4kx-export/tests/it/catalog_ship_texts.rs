// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Language, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{GlobalIniBuilder, TestEntry};
use serde_json::Value;

use super::{TestResult, catalog_with, options};
use crate::catalog_fixture;
use crate::text_forms_assertions::{absent, direct, forms, present};

#[test]
fn ship_texts_join_exact_first_component_in_their_own_domains() -> TestResult {
    let (_, catalog) = catalog_with(&options())?;
    let ship = &catalog["ships"][0];
    let cases = [
        (
            "name",
            "publicNames",
            "@vehicle_NameACAS_Ship",
            Some("Ace Hauler"),
        ),
        ("description", "gameTexts", "@vehicle_DescACAS_Ship", None),
        ("role", "publicNames", "@LOC_PLACEHOLDER", None),
        (
            "career",
            "publicNames",
            "@vehicle_focus_transporter",
            Some("Transporter"),
        ),
    ];
    for (field, domain, key, display) in cases {
        let cell = &ship[field];
        direct(cell)?;
        assert_eq!(cell["state"], "present", "{field} has its authored key");
        assert_eq!(
            cell["containsEncryptedSource"], true,
            "{field} uses the DCB"
        );
        let rows = catalog["texts"][domain].as_array().ok_or("text table")?;
        let mut matches = rows
            .iter()
            .filter(|row| row["id"]["value"] == cell["value"]);
        let row = matches.next().ok_or("text reference target")?;
        assert!(matches.next().is_none(), "the reference has one target");
        assert_eq!(
            row["key"]["value"], key,
            "{field} selects the first component"
        );
        let translation = forms(row, "en")?;
        match display {
            Some(display) => {
                present(translation, "sourceText", display)?;
                present(translation, "displayText", display)?;
            }
            None => absent(translation)?,
        }
    }
    verify_domain_limits(&catalog)
}

fn verify_domain_limits(catalog: &Value) -> TestResult {
    let public_names = catalog
        .pointer("/texts/publicNames")
        .and_then(Value::as_array)
        .ok_or("public names")?;
    assert!(
        public_names.iter().all(|name| {
            name.pointer("/key/value").and_then(Value::as_str)
                != Some("@fixture_unused_second_vehicle_name")
                && name.pointer("/key/value").and_then(Value::as_str)
                    != Some("@vehicle_DescACAS_Ship")
        }),
        "later components and protected description keys do not enter public names"
    );
    let rover = catalog["ships"]
        .as_array()
        .ok_or("ships")?
        .iter()
        .find(|entry| {
            entry.pointer("/className/value").and_then(Value::as_str) == Some("ACAS_Rover")
        })
        .ok_or("rover")?;
    let blank = &rover["career"];
    direct(blank)?;
    assert_eq!(blank["state"], "absent", "blank Locale is absent");
    assert!(
        blank["value"].is_null(),
        "blank Locale has no invented reference"
    );
    assert_eq!(
        blank["containsEncryptedSource"], true,
        "blank Locale preserves its DataForge evidence"
    );
    crate::health_origin_assertions::closure(catalog)?;
    Ok(())
}

#[test]
fn ship_name_has_english_german_and_french_display_forms() -> TestResult {
    let locale = |language: Language, name| {
        TestEntry::zstd(
            language.global_ini_path(),
            GlobalIniBuilder::new()
                .text("vehicle_NameACAS_Ship", name)
                .build(),
        )
    };
    let temp = tempfile::tempdir()?;
    let path = catalog_fixture::write_archive(
        temp.path(),
        &[
            catalog_fixture::english(),
            locale(Language::German, "Deutscher Name"),
            locale(Language::French, "Nom français"),
        ],
    )?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let built = build_catalog(&archive, &options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let name = &catalog["ships"][0]["name"]["value"];
    let row = catalog
        .pointer("/texts/publicNames")
        .and_then(Value::as_array)
        .ok_or("public names")?
        .iter()
        .find(|row| row["id"]["value"] == *name)
        .ok_or("ship name")?;
    for (language, expected) in [
        ("en", "Ace Hauler"),
        ("de", "Deutscher Name"),
        ("fr", "Nom français"),
    ] {
        let translation = forms(row, language)?;
        present(translation, "displayText", expected)?;
    }
    Ok(())
}
