// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::TestType;
use serde_json::Value;

use super::TestResult;
use crate::text_forms_assertions::{direct, forms, present};

#[path = "mission_type_fixture.rs"]
mod original;

pub(super) fn protected<'catalog>(
    catalog: &'catalog Value,
    reference: &Value,
) -> Result<&'catalog Value, Box<dyn Error>> {
    let identifier = reference.get("value").ok_or("text reference value")?;
    let rows = crate::health_origin_assertions::rows(catalog, "/texts/gameTexts")?;
    let mut matches = rows
        .iter()
        .filter(|row| row.pointer("/id/value") == Some(identifier));
    let row = matches.next().ok_or("protected mission type name target")?;
    assert!(
        matches.next().is_none(),
        "the reference has one protected target"
    );
    Ok(row)
}

#[test]
fn mission_type_accepts_original_locale_and_legacy_string_keys() -> TestResult {
    use crate::health_origin_assertions as proof;

    for storage in [TestType::Locale, TestType::String] {
        let catalog = fixture(storage)?;
        let [selected, placeholder, literal, empty] =
            proof::rows(&catalog, "/definitions/missionTypes/rows")?
        else {
            return Err("expected a selected, placeholder, literal and empty mission type".into());
        };
        assert!(
            catalog.get("missionTypes").is_none(),
            "the canonical definition has no parallel root array"
        );
        let guid = proof::present(
            selected,
            "guid",
            &serde_json::json!("00000000-0000-0000-0000-000000000001"),
            Some(true),
        )?;
        proof::header(&catalog, guid, 0, "guid", "payload", "archiveEntry")?;
        check_selected_name(&catalog, selected)?;

        for row in [placeholder, literal, empty] {
            let absent = proof::at(row, "/typeName")?;
            direct(absent)?;
            assert_eq!(
                proof::at(absent, "/state")?.as_str(),
                Some("absent"),
                "an unadmitted key has no text reference",
            );
            assert_eq!(
                proof::at(absent, "/containsEncryptedSource")?.as_bool(),
                Some(true),
                "an unadmitted key still cites the encrypted entry",
            );
        }
        assert_eq!(
            proof::rows(&catalog, "/texts/gameTexts")?.len(),
            1,
            "one protected key"
        );
        assert!(
            proof::rows(&catalog, "/texts/publicNames")?.is_empty(),
            "no public names"
        );
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn check_selected_name(catalog: &Value, selected: &Value) -> TestResult {
    use crate::health_origin_assertions as proof;

    let reference = proof::at(selected, "/typeName")?;
    direct(reference)?;
    assert_eq!(
        proof::at(reference, "/state")?.as_str(),
        Some("present"),
        "the selected mission type retains its key",
    );
    assert_eq!(
        proof::at(reference, "/containsEncryptedSource")?.as_bool(),
        Some(true),
        "the selected mission type name comes from the encrypted entry",
    );
    let row = protected(catalog, reference)?;
    assert_eq!(
        proof::at(row, "/key/value")?.as_str(),
        Some("@fixture_type"),
        "the stored mission type key",
    );
    let english = forms(row, "en")?;
    present(english, "sourceText", "A\\nMission")?;
    present(english, "displayText", "A\nMission")?;
    present(forms(row, "fr")?, "displayText", "Une mission")?;
    assert!(
        row.pointer("/translations/value/de").is_none(),
        "German has no source entry",
    );
    Ok(())
}

fn fixture(storage: TestType) -> Result<Value, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    original::write(&path, storage)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let output = build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
    Ok(serde_json::from_slice(output.json())?)
}
