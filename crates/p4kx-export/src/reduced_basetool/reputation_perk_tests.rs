// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, GlobalIniBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::reputation_catalog;
use super::selected_texts::SelectedTexts;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

#[path = "../../tests/it/reputation_perk_fixture.rs"]
mod fixture;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn reduced_perks_keep_authored_slots_variants_and_their_display_text() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    let mut input = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", fixture::complete().build()).encrypted());
    let languages = [Language::English, Language::German];
    for (language, name, description) in [
        (Language::English, "Fixture perk", "Fixture description"),
        (Language::German, "Testvorteil", "Testbeschreibung"),
    ] {
        input = input.entry(TestEntry::stored(
            language.global_ini_path(),
            GlobalIniBuilder::new()
                .text("shared", name)
                .text("own_description", description)
                .build(),
        ));
    }
    input.write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&languages),
        None,
    )?;
    let mut texts = SelectedTexts::default();
    let staged =
        reputation_catalog::collect(&session.parsed(None)?, scratch.path(), None, &mut texts)?;
    let perks = serde_json::to_value(staged.perks)?;
    let list = perks.pointer("/rows/0").ok_or("authored perk list")?;
    let entries = list
        .pointer("/perks/value")
        .and_then(Value::as_array)
        .ok_or("ordered perk entries")?;
    let [item, discount, dangling, authored, empty] = entries.as_slice() else {
        return Err("five original perk slots".into());
    };
    assert_eq!(
        item.pointer("/value/scope/value/targetRecordIndex/value"),
        Some(&json!(2))
    );
    assert_eq!(
        item.pointer("/value/standing/value/targetRecordIndex/value"),
        Some(&json!(3))
    );
    assert_eq!(
        item.pointer("/value/perk/value/details/rewardItem/value/targetRecordIndex/value"),
        Some(&json!(4))
    );
    assert_eq!(
        discount.pointer("/value/perk/value/details/discountPercentage/value"),
        Some(&json!({"type": "f32", "finite": 12.5}))
    );
    assert_eq!(
        dangling.pointer("/value/standing/value/status"),
        Some(&json!("dangling"))
    );
    assert_eq!(
        authored.pointer("/value/perk/value/details/type"),
        Some(&json!("authored"))
    );
    assert_eq!(
        authored.pointer("/value/perk/value/sourceType/value"),
        Some(&json!(fixture::AUTHORED))
    );
    assert!(
        authored
            .pointer("/value/perk/value/authoredFields")
            .is_none()
    );
    assert_eq!(empty.pointer("/value/perk/state"), Some(&json!("null")));
    let perk = item.pointer("/value/perk/value").ok_or("first perk body")?;
    assert_eq!(
        perk.pointer("/description/value"),
        Some(&json!(fixture::LITERAL))
    );
    verify_text(perk, &serde_json::to_value(texts)?)?;
    let factions = serde_json::to_value(staged.factions)?;
    assert_eq!(
        factions.pointer("/rows/0/perkReputationRewardList/value/0/value/targetRecordIndex/value"),
        list.pointer("/recordIndex/value")
    );
    Ok(())
}

fn verify_text(perk: &Value, texts: &Value) -> TestResult {
    let reference = perk
        .pointer("/displayDescription/value")
        .ok_or("description reference")?;
    let entries = texts
        .get("gameTexts")
        .and_then(Value::as_array)
        .ok_or("protected texts")?;
    let text = entries
        .iter()
        .find(|entry| entry.pointer("/id/value") == Some(reference))
        .ok_or("matching protected description")?;
    assert_eq!(
        text.pointer("/translations/value/de/displayText/value"),
        Some(&json!("Testbeschreibung"))
    );
    assert_eq!(
        text.pointer("/translations/value/en/displayText/value"),
        Some(&json!("Fixture description"))
    );
    Ok(())
}
