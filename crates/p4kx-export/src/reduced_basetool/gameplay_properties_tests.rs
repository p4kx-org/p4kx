// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

use super::ReducedError;
use super::gameplay_properties;
use super::selected_texts::SelectedTexts;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const PROPERTY: &str = "CraftingGameplayPropertyDef";
const BASE: &str = "CraftingDisplayTransformation_Base";
const SEQUENCE: &str = "CraftingDisplayTransformation_Sequence";
const PERCENT: &str = "CraftingDisplayTransformation_ConvertFactorToPercentChange";
const SCALE: &str = "CraftingDisplayTransformation_Scale";
const FUTURE: &str = "FixtureUnknownTransformation";
const OVERRIDE: &str = "CraftingPropertyNameOverride";
const CONDITION: &str = "CraftingPropertyNameOverrideCondition_Base";
const OWNER: u128 = 0x1177_0200;
const ENGLISH_FORMAT: &str = "  %+.2f\\n units  ";
const GERMAN_FORMAT: &str = "  %+.2f\\n Einheiten  ";

#[derive(Clone, Copy)]
enum Layout {
    Complete,
    UnknownOperation,
    WrongScaleStorage,
}

#[test]
fn keeps_selected_property_forms_and_flat_transformations() -> TestResult {
    with_queries(Layout::Complete, |parsed, scratch| {
        let mut texts = SelectedTexts::default();
        let output = gameplay_properties::collect(parsed, scratch, None, &mut texts)?;
        let serialized = serde_json::to_string(&output)?;
        let table: Value = serde_json::from_str(&serialized)?;
        let rows = at(&table, "/rows")?
            .as_array()
            .ok_or("gameplay property rows")?;
        let [property, alias] = rows.as_slice() else {
            return Err("one row per physical property Record".into());
        };
        assert_physical_records(property, alias);
        assert_transformations(at(property, "/displayTransformation/value")?)?;
        assert_text_closure(property, &serde_json::to_value(texts)?)?;
        for excluded in [
            "nameOverrides",
            "rawBits",
            "sourceUsesRefs",
            "propertyNameOverrides",
        ] {
            assert!(
                !serialized.contains(excluded),
                "the selected rows exclude {excluded}"
            );
        }
        Ok(())
    })
}

#[test]
fn incomplete_selected_display_transformations_refuse_reduced_publication() -> TestResult {
    for layout in [Layout::UnknownOperation, Layout::WrongScaleStorage] {
        with_queries(layout, |parsed, scratch| {
            let error =
                gameplay_properties::collect(parsed, scratch, None, &mut SelectedTexts::default())
                    .err();
            assert!(
                matches!(
                    error,
                    Some(ReducedError::IncompleteScope(gameplay_properties::PATH))
                ),
                "a selected unsupported operation or a wrong numeric declaration cannot be omitted"
            );
            Ok(())
        })?;
    }
    Ok(())
}

fn assert_physical_records(property: &Value, alias: &Value) {
    assert_eq!(property.pointer("/recordIndex/value"), Some(&json!(0)));
    assert_eq!(alias.pointer("/recordIndex/value"), Some(&json!(1)));
    assert_eq!(property.get("guid"), alias.get("guid"));
    assert_ne!(property.get("className"), alias.get("className"));
    assert_ne!(property.get("recordPath"), alias.get("recordPath"));
    assert_eq!(
        alias.pointer("/displayTransformation/state"),
        Some(&json!("null"))
    );
}

fn assert_transformations(transform: &Value) -> TestResult {
    assert_eq!(transform.pointer("/root/nodeIndex"), Some(&json!(0)));
    let nodes = at(transform, "/nodes")?
        .as_array()
        .ok_or("preorder nodes")?;
    let [sequence, scale, percentage] = nodes.as_slice() else {
        return Err("only three actual authored transformation occurrences".into());
    };
    assert_eq!(
        sequence.pointer("/sourceType/value"),
        Some(&json!(SEQUENCE))
    );
    assert_eq!(
        percentage.pointer("/payload/value/type"),
        Some(&json!("convertFactorToPercentChange"))
    );
    let slots = at(sequence, "/payload/value/transformations/value")?;
    assert_eq!(slots.pointer("/0/value/nodeIndex"), Some(&json!(1)));
    assert_eq!(slots.pointer("/1/state"), Some(&json!("null")));
    assert_eq!(slots.pointer("/1/value"), Some(&Value::Null));
    assert_eq!(slots.pointer("/2/value/nodeIndex"), Some(&json!(2)));
    let numeric = at(scale, "/payload/value/scale")?;
    assert_eq!(numeric.get("containsEncryptedSource"), Some(&json!(false)));
    assert_eq!(numeric.pointer("/value/type"), Some(&json!("f32")));
    assert_eq!(
        numeric
            .pointer("/value/finite")
            .and_then(Value::as_f64)
            .map(f64::to_bits),
        Some((-0.0_f64).to_bits())
    );
    Ok(())
}

fn assert_text_closure(property: &Value, texts: &Value) -> TestResult {
    let names = at(texts, "/publicNames")?
        .as_array()
        .ok_or("selected public names")?;
    assert_eq!(
        names.len(),
        2,
        "keys used only by omitted name overrides stay outside the table"
    );
    let name_id = at(property, "/propertyName/value")?;
    let unit_id = at(property, "/unitFormat/value")?;
    assert_ne!(name_id, unit_id);
    let name = public_name(names, name_id)?;
    let unit = public_name(names, unit_id)?;
    assert_eq!(
        name.pointer("/translations/value/en/displayText/value"),
        Some(&json!("Yield"))
    );
    assert_eq!(unit.pointer("/key/value"), Some(&json!("@fixture_unit")));
    assert_format(unit, "en", ENGLISH_FORMAT, "%+.2f\n units", false)?;
    assert_format(unit, "de", GERMAN_FORMAT, "%+.2f\n Einheiten", true)
}

fn assert_format(
    unit: &Value,
    language: &str,
    source: &str,
    display: &str,
    encrypted: bool,
) -> TestResult {
    let forms = at(unit, "/translations/value")?
        .get(language)
        .ok_or("selected translation")?;
    for (field, expected) in [("sourceText", source), ("displayText", display)] {
        let cell = forms.get(field).ok_or("source or display form")?;
        assert_eq!(cell.get("value"), Some(&json!(expected)));
        assert_eq!(cell.get("containsEncryptedSource"), Some(&json!(encrypted)));
    }
    Ok(())
}

fn public_name<'read>(nodes: &'read [Value], id: &Value) -> TestResult<&'read Value> {
    nodes
        .iter()
        .find(|row| row.pointer("/id/value") == Some(id))
        .ok_or_else(|| "selected public name has a matching definition".into())
}

fn at<'read>(row: &'read Value, pointer: &str) -> TestResult<&'read Value> {
    row.pointer(pointer)
        .ok_or_else(|| format!("missing gameplay property field {pointer}").into())
}

fn with_queries(
    layout: Layout,
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            database(layout).build(),
        ))
        .entry(locale(Language::English, "Yield", ENGLISH_FORMAT))
        .entry(locale(Language::German, "Ertrag", GERMAN_FORMAT).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    inspect(&session.parsed(None)?, scratch.path())
}

fn locale(language: Language, name: &str, unit: &str) -> TestEntry {
    TestEntry::stored(
        language.global_ini_path(),
        format!("fixture_name={name}\nfixture_unit={unit}\nfixture_override=Unused\n").into_bytes(),
    )
}

fn database(layout: Layout) -> DataForgeBuilder {
    let storage = match layout {
        Layout::Complete | Layout::UnknownOperation => TestType::Single,
        Layout::WrongScaleStorage => TestType::Double,
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(PROPERTY)
                .field("propertyName", TestType::Locale)
                .field("unitFormat", TestType::Locale)
                .field("displayTransformation", pointer(BASE))
                .array("nameOverrides", TestType::Class(OVERRIDE.to_owned())),
        )
        .structure(
            TestStruct::new(OVERRIDE)
                .field("propertyName", TestType::Locale)
                .field("condition", pointer(CONDITION)),
        )
        .structure(TestStruct::new(CONDITION))
        .structure(TestStruct::new(BASE))
        .structure(TestStruct::new(PERCENT).parent(BASE))
        .structure(TestStruct::new(FUTURE).parent(BASE))
        .structure(TestStruct::new(SCALE).parent(BASE).field("scale", storage))
        .structure(
            TestStruct::new(SEQUENCE)
                .parent(BASE)
                .array("transformations", pointer(BASE)),
        )
        .record(record("Primary", transformation(layout)))
        .record(record("Alias", TestValue::Null))
}

fn record(name: &str, transformation: TestValue) -> TestRecord {
    let override_name = TestInstance::new(OVERRIDE)
        .value("propertyName", TestValue::text("@fixture_override"))
        .value("condition", instance(TestInstance::new(CONDITION)));
    TestRecord::new(
        OWNER,
        PROPERTY,
        format!("{PROPERTY}.{name}"),
        format!("libs/foundry/records/crafting/craftedproperties/{name}.xml"),
    )
    .value("propertyName", TestValue::text("@fixture_name"))
    .value("unitFormat", TestValue::text("@fixture_unit"))
    .value("displayTransformation", transformation)
    .value(
        "nameOverrides",
        TestValue::Array(vec![instance(override_name)]),
    )
}

fn transformation(layout: Layout) -> TestValue {
    match layout {
        Layout::Complete => {
            let values = vec![
                scale(TestValue::Single(-0.0)),
                TestValue::Null,
                instance(TestInstance::new(PERCENT)),
            ];
            instance(TestInstance::new(SEQUENCE).value("transformations", TestValue::Array(values)))
        }
        Layout::UnknownOperation => instance(TestInstance::new(FUTURE)),
        Layout::WrongScaleStorage => scale(TestValue::Double(0.5)),
    }
}

fn scale(value: TestValue) -> TestValue {
    instance(TestInstance::new(SCALE).value("scale", value))
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

const fn instance(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
