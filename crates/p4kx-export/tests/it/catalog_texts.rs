// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! English and German names and descriptions in the Basetool catalog, through the public API.

#[path = "catalog_text_values.rs"]
mod values;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, Language, TextSummary, build_catalog, build_catalog_from};
use p4kx_extraction::{
    Archive, DataForgeEntry, IniText, LanguageTexts, Resolution, TextResolver, TextSet,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);

fn manufacturer(guid: u128, class: &str, name: &str, description: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "SCItemManufacturer",
        format!("SCItemManufacturer.{class}"),
        format!("libs/foundry/records/scitemmanufacturer/{class}.xml"),
    )
    .value("Code", TestValue::text(class))
    .value(
        "Localization",
        TestValue::Struct(
            TestInstance::new("SCItemLocalization")
                .value("Name", TestValue::text(name))
                .value("Description", TestValue::text(description)),
        ),
    )
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("SCItemLocalization")
                .field("Name", TestType::Locale)
                .field("Description", TestType::Locale),
        )
        .structure(
            TestStruct::new("SCItemManufacturer")
                .field("Code", TestType::String)
                .field(
                    "Localization",
                    TestType::Class("SCItemLocalization".to_owned()),
                ),
        )
        .record(manufacturer(
            0x1000_0000_0000_4000_8000_0000_0000_0001,
            "ACAS",
            "@manufacturer_NameACAS",
            "@manufacturer_DescACAS",
        ))
        .record(manufacturer(
            0x2000_0000_0000_4000_8000_0000_0000_0002,
            "DRAK",
            "@manufacturer_NameDRAK",
            "",
        ))
        .record(manufacturer(
            0x3000_0000_0000_4000_8000_0000_0000_0003,
            "MISC",
            "@loc_placeholder",
            "@manufacturer_DescMISC",
        ))
        .build()
}

fn english() -> TestEntry {
    TestEntry::zstd(
        Language::English.global_ini_path(),
        GlobalIniBuilder::new()
            .text("manufacturer_NameACAS", "Ace Astrogation")
            .text("manufacturer_DescACAS", "Power plants.")
            .text("manufacturer_NameDRAK", "Drake Interplanetary")
            .text("LOC_PLACEHOLDER", "<= PLACEHOLDER =>")
            .text("manufacturer_DescMISC", "Line one\\nLine two\rkept")
            .build(),
    )
}

fn german() -> TestEntry {
    TestEntry::zstd(
        Language::German.global_ini_path(),
        GlobalIniBuilder::new()
            .text("manufacturer_NameACAS", "Ace Astrogation")
            .text("manufacturer_DescACAS", "Kraftwerke.")
            .text("manufacturer_DescMISC", "")
            .build(),
    )
}

fn catalog(entries: Vec<TestEntry>) -> Result<(Value, Vec<TextSummary>), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    entries
        .into_iter()
        .fold(
            ArchiveBuilder::new().entry(TestEntry::zstd("Data\\Game2.dcb", database())),
            ArchiveBuilder::entry,
        )
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let built = build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT))?;
    let json = serde_json::from_slice(built.json())?;
    Ok((json, built.summary().texts().to_vec()))
}

fn text<'c>(catalog: &'c Value, index: usize, field: &str) -> Result<&'c Value, Box<dyn Error>> {
    let row = catalog
        .pointer("/definitions/manufacturers/rows")
        .and_then(|rows| rows.get(index))
        .ok_or("manufacturer")?;
    let localization = row.get("localization").ok_or("localization carrier")?;
    assert_eq!(
        localization.get("state"),
        Some(&json!("present")),
        "localization is present"
    );
    let reference = localization
        .get("value")
        .and_then(|value| value.get(field))
        .ok_or("text reference carrier")?;
    assert_eq!(
        reference.get("state"),
        Some(&json!("present")),
        "the authored text reference is present"
    );
    let target = reference
        .get("value")
        .and_then(Value::as_str)
        .ok_or("text reference")?;
    let domain = match field {
        "name" => "publicNames",
        "description" => "gameTexts",
        _ => return Err("unknown text domain".into()),
    };
    let rows = catalog
        .get("texts")
        .and_then(|tables| tables.get(domain))
        .and_then(Value::as_array)
        .ok_or("canonical text table")?;
    let mut matching = rows
        .iter()
        .filter(|text| text.pointer("/id/value").and_then(Value::as_str) == Some(target));
    let text = matching.next().ok_or("canonical text reference")?;
    assert!(matching.next().is_none(), "unique text identity");
    assert_eq!(
        text.pointer("/key/state"),
        Some(&json!("present")),
        "the canonical text key is present"
    );
    Ok(text)
}

#[test]
fn names_and_descriptions_hold_canonical_keys_and_both_resolved_texts() -> TestResult {
    let (catalog, _) = catalog(vec![english(), german()])?;
    assert!(catalog.get("manufacturers").is_none());
    let name = text(&catalog, 0, "name")?;
    assert_eq!(name["key"]["value"], "@manufacturer_NameACAS");
    values::translation(name, "en", "present", &json!("Ace Astrogation"))?;
    values::translation(name, "de", "present", &json!("Ace Astrogation"))?;
    let description = text(&catalog, 0, "description")?;
    assert_eq!(description["key"]["value"], "@manufacturer_DescACAS");
    values::translation(description, "en", "present", &json!("Power plants."))?;
    values::translation(description, "de", "present", &json!("Kraftwerke."))?;
    Ok(())
}

#[test]
fn missing_keys_keep_canonical_keys_and_explicit_absent_translations() -> TestResult {
    let (catalog, summary) = catalog(vec![english(), german()])?;
    let name = text(&catalog, 1, "name")?;
    assert_eq!(name["key"]["value"], "@manufacturer_NameDRAK");
    values::translation(name, "en", "present", &json!("Drake Interplanetary"))?;
    values::translation(name, "de", "absent", &Value::Null)?;
    let description = text(&catalog, 1, "description")?;
    assert_eq!(
        description["key"]["value"], "",
        "authored empty Locale is retained"
    );
    values::translation(description, "en", "absent", &Value::Null)?;
    values::translation(description, "de", "absent", &Value::Null)?;
    assert_eq!(
        summary,
        [
            TextSummary {
                language: Language::English,
                available: true,
                resolved: 4,
                unresolved: 1,
                placeholder: 1
            },
            TextSummary {
                language: Language::German,
                available: true,
                resolved: 2,
                unresolved: 3,
                placeholder: 1
            },
        ]
    );
    Ok(())
}

#[test]
fn a_missing_language_has_no_fabricated_translation_carrier_and_counts_every_key() -> TestResult {
    let (catalog, summary) = catalog(vec![english()])?;
    let name = text(&catalog, 0, "name")?;
    values::translation(name, "en", "present", &json!("Ace Astrogation"))?;
    assert!(
        name["translations"]["value"].get("de").is_none(),
        "no German input"
    );
    assert_eq!(
        summary.get(1),
        Some(&TextSummary {
            language: Language::German,
            available: false,
            resolved: 0,
            unresolved: 5,
            placeholder: 1,
        })
    );
    Ok(())
}

#[test]
fn a_placeholder_key_is_retained_without_lookup_and_counted_apart() -> TestResult {
    let (catalog, summary) = catalog(vec![english(), german()])?;
    let name = text(&catalog, 2, "name")?;
    assert_eq!(name["key"]["value"], "@loc_placeholder");
    values::translation(name, "en", "absent", &Value::Null)?;
    values::translation(name, "de", "absent", &Value::Null)?;
    assert!(summary.iter().all(|texts| texts.placeholder == 1));
    Ok(())
}

#[test]
fn an_authored_empty_translation_is_present_and_unresolved() -> TestResult {
    let (catalog, _) = catalog(vec![english(), german()])?;
    values::translation(
        text(&catalog, 2, "description")?,
        "de",
        "present",
        &json!(""),
    )
}

#[test]
fn the_line_break_escape_becomes_a_line_feed_and_a_lone_cr_stays() -> TestResult {
    let (catalog, _) = catalog(vec![english(), german()])?;
    values::forms_translation(
        text(&catalog, 2, "description")?,
        "en",
        "present",
        &json!("Line one\\nLine two\rkept"),
        &json!("Line one\nLine two\rkept"),
    )
}

#[test]
fn a_catalog_from_a_read_entry_equals_one_read_from_the_archive() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database()))
        .entry(english())
        .entry(german())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let options = CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT);

    let read = build_catalog(&archive, &options)?;
    let entry = DataForgeEntry::read(&archive)?;
    let from = build_catalog_from(&archive, &entry, &options)?;

    assert_eq!(from.json(), read.json(), "identical bytes");
    assert_eq!(from.summary(), read.summary(), "identical summary");
    Ok(())
}

#[test]
fn texts_are_trimmed_in_the_catalog_and_stay_padded_in_the_raw_resolver() -> TestResult {
    let padded = GlobalIniBuilder::new()
        .text("manufacturer_NameACAS", " Ace Astrogation \\n")
        .text("manufacturer_DescACAS", "\\n  \\n")
        .text("manufacturer_NameDRAK", "Drake\\nInterplanetary ")
        .build();
    let entry = TestEntry::zstd(Language::English.global_ini_path(), padded.clone());
    let (catalog, summary) = catalog(vec![entry])?;
    values::forms_translation(
        text(&catalog, 0, "name")?,
        "en",
        "present",
        &json!(" Ace Astrogation \\n"),
        &json!("Ace Astrogation"),
    )?;
    values::forms_translation(
        text(&catalog, 0, "description")?,
        "en",
        "present",
        &json!("\\n  \\n"),
        &json!(""),
    )?;
    values::forms_translation(
        text(&catalog, 1, "name")?,
        "en",
        "present",
        &json!("Drake\\nInterplanetary "),
        &json!("Drake\nInterplanetary"),
    )?;
    assert_eq!(
        summary.first().map(|texts| texts.unresolved),
        Some(3),
        "empty authored key and white-space-only/missing texts remain unresolved"
    );
    let text = IniText::decode(&padded)?;
    let set = TextSet::new([(Language::English, text.index()?)]);
    let raw = LanguageTexts::new(&set, Language::English);
    assert_eq!(
        TextResolver::resolve(&raw, "@manufacturer_NameACAS"),
        Resolution::Text(" Ace Astrogation \n".into()),
        "the raw resolver keeps the padding"
    );
    Ok(())
}
