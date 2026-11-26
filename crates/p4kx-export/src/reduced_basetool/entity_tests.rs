// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestStruct, TestType, TestValue,
};
use serde_json::Value;

use super::entities;
use super::selected_texts::SelectedTexts;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};
use crate::ship_store_pledge;
use crate::ship_store_slugs::StoreSlugs;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const KEY: &str = "fixture_Name";
const MAKER_GUID: &str = "00000000-0000-0000-0000-000000000001";
const LOCALE: &str = "FixtureManufacturerLocalization";

#[test]
fn selected_entity_tables_preserve_bilingual_domains_and_reviewed_pledges() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = archive(scratch.path())?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    let mut texts = SelectedTexts::default();
    let slice = entities::collect(&session.parsed(None)?, scratch.path(), None, &mut texts)?;
    assert_eq!(slice.items.len(), 0, "no Item records in fixture");
    assert_eq!(slice.item_definitions.len(), 0, "no Item definition rows");
    assert_eq!(slice.ships.len(), 0, "no Ship records in fixture");
    assert!(
        slice.ship_cargo.is_empty(),
        "no canonical cargo rows without Ships"
    );
    assert_eq!(slice.paints.len(), 0, "no Paint records in fixture");
    let mapping = StoreSlugs::embedded()?;
    assert_eq!(
        serde_json::to_value(&slice.ship_store_slugs)?,
        serde_json::to_value(mapping.resolve(std::iter::empty(), false).exported)?,
        "a complete empty Ship survey produces the canonical Store selection"
    );
    assert_eq!(
        slice.ship_store_pledges,
        ship_store_pledge::ship_store_pledges()?,
        "the external reviewed snapshot is independent of game rows"
    );
    verify_maker_and_texts(
        &serde_json::to_value(&slice.manufacturers)?,
        &serde_json::to_value(&texts)?,
    )
}

fn verify_maker_and_texts(makers: &Value, texts: &Value) -> TestResult {
    assert_eq!(at(makers, "/rows/0/guid/value")?, MAKER_GUID);
    assert_eq!(at(makers, "/rows/0/recordIndex/value")?, 0);
    assert_eq!(at(makers, "/rows/0/code/value")?, "FIXTURE");
    assert!(
        makers.pointer("/rows/0/logo").is_none(),
        "an unselected raw field is omitted"
    );

    let public = at(texts, "/publicNames/0")?;
    let protected = at(texts, "/gameTexts/0")?;
    assert_ne!(
        at(public, "/id/value")?,
        at(protected, "/id/value")?,
        "the same localization key remains in both original domains"
    );
    for row in [public, protected] {
        assert_eq!(at(row, "/key/value")?, KEY);
        assert_eq!(
            at(row, "/translations/value/en/sourceText/value")?,
            "Fixture name"
        );
        assert_eq!(
            at(row, "/translations/value/de/sourceText/state")?,
            "absent"
        );
        assert!(
            row.pointer("/translations/value/fr").is_none(),
            "unselected locales are omitted"
        );
    }
    assert_eq!(
        at(makers, "/rows/0/localization/value/name/value")?,
        at(public, "/id/value")?
    );
    assert_eq!(
        at(makers, "/rows/0/localization/value/description/value")?,
        at(protected, "/id/value")?
    );
    Ok(())
}

fn archive(directory: &Path) -> TestResult<Archive> {
    let path = directory.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()))
        .entry(localization(Language::English, KEY, "Fixture name"))
        .entry(localization(Language::German, "unrelated", "Unbeteiligt"))
        .entry(localization(Language::French, KEY, "Nom de test"))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}

fn database() -> Vec<u8> {
    let localization = TestInstance::new(LOCALE)
        .value("Name", TestValue::text(KEY))
        .value("ShortName", TestValue::text(KEY))
        .value("Description", TestValue::text(KEY));
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("SCItemManufacturer")
                .field("Code", TestType::String)
                .field("Logo", TestType::String)
                .field("Localization", TestType::Class(LOCALE.to_owned())),
        )
        .structure(
            TestStruct::new(LOCALE)
                .field("Name", TestType::Locale)
                .field("ShortName", TestType::Locale)
                .field("Description", TestType::Locale),
        )
        .record(
            TestRecord::new(
                1,
                "SCItemManufacturer",
                "SCItemManufacturer.Fixture",
                "libs/foundry/records/scitemmanufacturer/fixture.xml",
            )
            .value("Code", TestValue::text("FIXTURE"))
            .value("Logo", TestValue::text("excluded"))
            .value("Localization", TestValue::Struct(localization)),
        )
        .build()
}

fn localization(language: Language, key: &str, value: &str) -> TestEntry {
    TestEntry::stored(
        language.global_ini_path(),
        GlobalIniBuilder::new().text(key, value).build(),
    )
}

fn at<'value>(item: &'value Value, path: &str) -> TestResult<&'value Value> {
    item.pointer(path)
        .ok_or_else(|| format!("missing {path}").into())
}
