// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestStruct, TestType, TestValue, test_key,
};

use super::contract_scope;
use super::selected_texts::SelectedTexts;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ROOT: &str = "ContractScopeGlobalParams";
const ITEM: &str = "ContractScope_Item";
const NAME: &str = "fixture_scope";

#[test]
fn contract_scope_retains_labels_and_localization_with_null_and_absent_slots() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()).encrypted())
        .entry(localization(Language::English, "Location"))
        .entry(localization(Language::German, "Ort"))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    let mut texts = SelectedTexts::default();
    let table = contract_scope::collect(&session.parsed(None)?, scratch.path(), None, &mut texts)?;
    let table = serde_json::to_value(table)?;
    let texts = serde_json::to_value(texts)?;
    let row = table.pointer("/rows/0").ok_or("contract scope row")?;
    assert_eq!(
        row["location"]["value"]["displayName"]["containsEncryptedSource"],
        true
    );
    assert_eq!(row["cluster"]["state"], "null");
    assert_eq!(row["universe"]["state"], "absent");
    assert!(row.get("additionalFields").is_none());
    assert!(row["location"]["value"].get("icon").is_none());

    let name = texts.pointer("/publicNames/0").ok_or("selected label")?;
    assert_eq!(
        row["location"]["value"]["displayName"]["value"],
        name["id"]["value"]
    );
    assert_eq!(
        name["translations"]["value"]["en"]["displayText"]["value"],
        "Location"
    );
    assert_eq!(
        name["translations"]["value"]["de"]["displayText"]["value"],
        "Ort"
    );
    assert_eq!(texts["gameTexts"].as_array().map(Vec::len), Some(0));
    Ok(())
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(ROOT)
                .field("location", TestType::StrongPointer(ITEM.to_owned()))
                .field("cluster", TestType::StrongPointer(ITEM.to_owned()))
                .field("fixtureRootExtra", TestType::Bool),
        )
        .structure(
            TestStruct::new(ITEM)
                .field("displayName", TestType::Locale)
                .field("icon", TestType::String),
        )
        .record(
            TestRecord::new(
                0x1185,
                ROOT,
                "ContractScopeGlobalParams.Fixture",
                "fixture/scope.xml",
            )
            .value(
                "location",
                TestValue::Struct(
                    TestInstance::new(ITEM)
                        .value("displayName", TestValue::text(format!("@{NAME}")))
                        .value("icon", TestValue::text("Fixture/Scope/Icon")),
                ),
            )
            .value("fixtureRootExtra", TestValue::Bool(true)),
        )
        .build()
}

fn localization(language: Language, label: &str) -> TestEntry {
    TestEntry::stored(
        language.global_ini_path(),
        GlobalIniBuilder::new()
            .text(NAME, label)
            .text("unused", "Unused")
            .build(),
    )
}
