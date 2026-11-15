// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, Language, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct,
    TestType, TestValue,
};
use serde_json::Value;

use super::TestResult;
use crate::text_forms_assertions::{direct, forms, present};

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
        let [selected, placeholder, literal, empty] = proof::rows(&catalog, "/missionTypes")?
        else {
            return Err("expected a selected, placeholder, literal and empty mission type".into());
        };
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
    let mut database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("MissionType").field("LocalisedTypeName", storage));
    for (ordinal, key) in [
        (1, "@fixture_type"),
        (2, "@LOC_PLACEHOLDER"),
        (3, "literal"),
        (4, ""),
    ] {
        database = database.record(
            TestRecord::new(
                ordinal,
                "MissionType",
                format!("MissionType.missiontype.fixture{ordinal}"),
                format!("libs/foundry/records/missiontype/fixture{ordinal}.xml"),
            )
            .value("LocalisedTypeName", TestValue::text(key)),
        );
    }
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let locale = |language: Language, value| {
        TestEntry::stored(
            language.global_ini_path(),
            GlobalIniBuilder::new().text("fixture_type", value).build(),
        )
    };
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()).encrypted())
        .entry(locale(Language::English, "A\\nMission"))
        .entry(locale(Language::French, "Une mission"))
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let output = build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
    Ok(serde_json::from_slice(output.json())?)
}
