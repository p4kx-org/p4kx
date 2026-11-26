// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::ReducedError;
use super::item_definitions;
use super::manufacturer_definitions;
use super::record_targets::RecordTargets;
use super::selected_texts::SelectedTexts;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

#[path = "item_definitions_fixture.rs"]
mod fixture;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn reduced_item_definitions_keep_canonical_order_physical_slots_and_text_domains() -> TestResult {
    with_queries(&fixture::database(), |parsed, scratch| {
        let mut texts = SelectedTexts::default();
        let (makers, targets) =
            manufacturer_definitions::collect(parsed, scratch, None, &mut texts)?;
        let reduced = item_definitions::collect(parsed, scratch, None, &mut texts, &targets)?;
        let exact_order =
            parsed.with_item_definitions(CatalogRecordSelection::All, None, |snapshot| {
                snapshot
                    .rows()
                    .iter()
                    .map(|row| row.source().record_index().value().copied())
                    .collect::<Vec<_>>()
            })?;
        assert_eq!(
            exact_order,
            [Some(1), Some(0), Some(2)],
            "canonical sorted GUID and physical aliases"
        );
        let reduced = serde_json::to_value(reduced)?;
        let items = rows(&reduced)?;
        let [low, first, alias] = items else {
            return Err("the complete original Item census".into());
        };
        for (row, index) in [(low, 1), (first, 0), (alias, 2)] {
            assert_eq!(row.pointer("/recordIndex/value"), Some(&json!(index)));
            assert!(row.get("itemClass").is_none());
            assert!(row.get("variantOf").is_none());
        }
        assert_eq!(first.pointer("/guid/value"), alias.pointer("/guid/value"));
        let makers = serde_json::to_value(makers)?;
        let texts = serde_json::to_value(texts)?;
        verify_components(first, rows(&makers)?, &texts)
    })
}

fn verify_components(item: &Value, makers: &[Value], texts: &Value) -> TestResult {
    let components = item
        .pointer("/components/value")
        .and_then(Value::as_array)
        .ok_or("projected components")?;
    let [attachment, physics, second_attachment, purchase] = components.as_slice() else {
        return Err("all four selected physical component rows".into());
    };
    for (component, ordinal) in [
        (attachment, 1),
        (physics, 2),
        (second_attachment, 3),
        (purchase, 4),
    ] {
        assert_eq!(
            component.pointer("/componentOrdinal/value"),
            Some(&json!(ordinal))
        );
        assert!(component.get("sourceType").is_none());
    }
    let original = attachment
        .pointer("/value/value")
        .ok_or("original AttachDef")?;
    verify_attachment(original, makers)?;
    assert_eq!(
        second_attachment.pointer("/value/value/manufacturer/value/state"),
        Some(&json!("null")),
        "explicit null manufacturer is preserved on the second slot"
    );
    let mass = physics
        .pointer("/value/value/mass/value")
        .ok_or("original mass")?;
    assert_eq!(
        serde_json::to_string(mass)?,
        r#"{"finite":-0.0,"type":"f32"}"#
    );
    verify_texts(texts, original, purchase)
}

fn verify_attachment(original: &Value, makers: &[Value]) -> TestResult {
    for (field, value) in [
        ("itemType", "WeaponPersonal"),
        ("itemSubtype", "Rifle"),
        ("tags", "fixture fit tags"),
        ("requiredTags", "fixture required tags"),
    ] {
        let key = format!("/{field}/value");
        assert_eq!(
            original.pointer(&key),
            Some(&json!(value)),
            "{field} retains authored text"
        );
    }
    assert_eq!(
        original.pointer("/size/value"),
        Some(&json!({"type":"uint16", "value":3}))
    );
    assert_eq!(
        original.pointer("/grade/value"),
        Some(&json!({"type":"int32", "value":2}))
    );
    let target = original
        .pointer("/manufacturer/value/value")
        .ok_or("physical manufacturer target")?;
    let index = target
        .pointer("/targetRecordIndex/value")
        .ok_or("original manufacturer record position")?;
    let selected = makers
        .iter()
        .find(|maker| maker.pointer("/recordIndex/value") == Some(index))
        .ok_or("the selected physical manufacturer's row")?;
    assert_eq!(selected.get("guid"), target.get("guid"));
    assert_eq!(target.get("status"), Some(&json!("present")));
    assert_eq!(
        original.pointer("/manufacturer/value/containsEncryptedSource"),
        Some(&json!(true))
    );
    assert!(original.get("weaponMotion").is_none());
    Ok(())
}

fn verify_texts(texts: &Value, attachment: &Value, purchase: &Value) -> TestResult {
    let references = [
        (
            "publicNames",
            attachment.pointer("/localization/value/name/value"),
        ),
        (
            "gameTexts",
            attachment.pointer("/localization/value/description/value"),
        ),
        ("publicNames", purchase.pointer("/value/displayName/value")),
        ("gameTexts", purchase.pointer("/value/displayType/value")),
    ];
    for (domain, id) in references {
        let id = id.ok_or("selected text reference")?;
        let rows = texts
            .get(domain)
            .and_then(Value::as_array)
            .ok_or("selected text domain")?;
        assert_eq!(
            rows.len(),
            2,
            "{domain} retains only referenced keys and merges aliases"
        );
        let row = rows
            .iter()
            .find(|row| row.pointer("/id/value") == Some(id))
            .ok_or("closed text reference")?;
        for (locale, expected) in [("en", "English text"), ("de", "Deutscher Text")] {
            let pointer = format!("/translations/value/{locale}/displayText/value");
            assert_eq!(row.pointer(&pointer), Some(&json!(expected)));
        }
    }
    assert_ne!(
        attachment.pointer("/localization/value/name/value"),
        attachment.pointer("/localization/value/description/value"),
        "the same key retains its separate public and protected identifiers"
    );
    Ok(())
}

#[test]
fn reduced_item_definitions_reject_unclosed_and_nonrecord_manufacturer_shapes() -> TestResult {
    with_queries(&fixture::database(), |parsed, scratch| {
        let error = item_definitions::collect(
            parsed,
            scratch,
            None,
            &mut SelectedTexts::default(),
            &RecordTargets::default(),
        );
        assert!(
            matches!(
                error,
                Err(ReducedError::UnresolvedRecordLink {
                    path: manufacturer_definitions::PATH,
                    index: 3 | 4
                })
            ),
            "a resolved source cannot point to an omitted physical Manufacturer"
        );
        Ok(())
    })?;
    with_queries(&fixture::authored_manufacturer(), |parsed, scratch| {
        let error = item_definitions::collect(
            parsed,
            scratch,
            None,
            &mut SelectedTexts::default(),
            &RecordTargets::default(),
        );
        assert!(
            matches!(error, Err(ReducedError::UnsupportedManufacturer(0))),
            "a non-reference source cannot be made into a manufacturer Record"
        );
        Ok(())
    })
}

fn rows(table: &Value) -> Result<&[Value], &'static str> {
    table
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or("staged definition rows")
}

fn with_queries(
    database: &DataForgeBuilder,
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted());
    let languages = [Language::English, Language::German];
    let keys = [
        fixture::SHARED_TEXT,
        fixture::DISPLAY_NAME,
        fixture::DISPLAY_TYPE,
        "unselected_fixture_text",
    ];
    for (language, text) in languages
        .into_iter()
        .zip(["English text", "Deutscher Text"])
    {
        let ini = keys
            .iter()
            .fold(GlobalIniBuilder::new(), |ini, key| ini.text(*key, text))
            .build();
        archive = archive.entry(TestEntry::stored(language.global_ini_path(), ini));
    }
    archive.write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&languages),
        None,
    )?;
    inspect(&session.parsed(None)?, scratch.path())
}
