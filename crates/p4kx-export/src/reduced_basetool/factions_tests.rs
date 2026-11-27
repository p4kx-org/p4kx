// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct,
    TestType, TestValue, test_key,
};
use serde_json::{Value, json};

use super::factions;
use super::record_targets::RecordTargets;
use super::selected_texts::SelectedTexts;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn factions_keep_physical_relationships_and_selected_public_names() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    let text = GlobalIniBuilder::new()
        .text("agency", "Security")
        .text("omitted", "Unselected description")
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()).encrypted())
        .entry(TestEntry::stored(Language::English.global_ini_path(), text))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    let mut texts = SelectedTexts::default();
    let staged = factions::collect(
        &session.parsed(None)?,
        &RecordTargets::default(),
        scratch.path(),
        None,
        &mut texts,
    )?;
    let table = serde_json::to_value(&staged.table)?;
    let rows = table
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("ordered factions")?;
    assert_eq!(rows.len(), 2, "both physical factions are present");
    assert_eq!(
        staged.targets.iter().count(),
        rows.len(),
        "all selected relationship targets are staged"
    );
    let agency = rows
        .iter()
        .find(|row| row.pointer("/ableToArrest/value") == Some(&json!(true)))
        .ok_or("security authority")?;
    verify_relationships(agency)?;
    verify_text(agency, &serde_json::to_value(texts)?)
}

fn verify_relationships(agency: &Value) -> TestResult {
    assert_eq!(
        agency.pointer("/ableToArrest/containsEncryptedSource"),
        Some(&json!(true)),
        "original DataForge determines encryption"
    );
    let entries = agency
        .pointer("/alliedFactions/value")
        .and_then(Value::as_array)
        .ok_or("ordered ally slots")?;
    let [first, dangling, repeated] = entries.as_slice() else {
        return Err("three ally slots".into());
    };
    assert_eq!(
        first.pointer("/value/targetRecordIndex/value"),
        Some(&json!(1)),
        "first ally resolves to the original physical record"
    );
    assert_eq!(first, repeated, "duplicate authoring slots survive");
    assert_eq!(
        dangling.pointer("/value/status"),
        Some(&json!("dangling")),
        "unresolved reference remains explicit"
    );
    assert!(
        agency.get("authoredFields").is_none(),
        "raw arena is outside the scope"
    );
    Ok(())
}

fn verify_text(agency: &Value, texts: &Value) -> TestResult {
    assert!(
        agency.get("description").is_none(),
        "description is outside the selected Faction fields"
    );
    let name = agency
        .pointer("/name/value/reference")
        .ok_or("public name reference")?;
    assert_eq!(
        texts.pointer("/publicNames/0/id/value"),
        Some(name),
        "reference closes over the selected public name"
    );
    assert_eq!(
        texts.pointer("/publicNames/0/translations/value/en/displayText/value"),
        Some(&json!("Security")),
        "selected translation is preserved"
    );
    assert_eq!(
        texts.get("gameTexts"),
        Some(&json!([])),
        "unselected text is excluded"
    );
    Ok(())
}

fn database() -> DataForgeBuilder {
    const FACTION: &str = "Faction";
    const AUTHORITY: u128 = 0x1234;
    const ALLY: u128 = 0x1235;
    const MISSING: u128 = 0x1236;
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(FACTION)
                .field("name", TestType::Locale)
                .field("description", TestType::Locale)
                .field("ableToArrest", TestType::Bool)
                .array("alliedFactions", TestType::ReferenceTo(FACTION.to_owned())),
        )
        .record(
            TestRecord::new(
                AUTHORITY,
                FACTION,
                "Faction.Authority",
                "libs/foundry/records/factions/authority.xml",
            )
            .value("name", TestValue::text("@agency"))
            .value("description", TestValue::text("@omitted"))
            .value("ableToArrest", TestValue::Bool(true))
            .value(
                "alliedFactions",
                TestValue::Array(
                    [ALLY, MISSING, ALLY]
                        .into_iter()
                        .map(TestValue::Reference)
                        .collect(),
                ),
            ),
        )
        .record(TestRecord::new(
            ALLY,
            FACTION,
            "Faction.Ally",
            "libs/foundry/records/factions/ally.xml",
        ))
}
