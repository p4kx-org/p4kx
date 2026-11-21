// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_export::{BuiltCatalog, Kind};
use p4kx_extraction::{DataForge, Language, Value as Raw};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::catalog_crafting_support;
use crate::gameplay_v3_consumer as consumer;
use crate::quality_v3_origin_support::{self as origins, Domain, Use};
use crate::text_forms_assertions as forms;
use crate::text_forms_origin_assertions::{self as text_origins, Lookup};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const KIND: Kind = Kind::GameplayProperties;

pub(crate) struct OriginalProperty {
    index: u32,
    guid: String,
    source_count: usize,
}

/// Checks the existing fixture's actual owner and Locale before exporting it.
pub(crate) fn original(
    database: &DataForgeBuilder,
    guid: u128,
    key: &str,
) -> TestResult<OriginalProperty> {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let expected = catalog_crafting_support::text(guid);
    let mut owner = None;
    let mut source_count = 0;
    for record in forge.records() {
        source_count += usize::from(record.type_name()? == "CraftingGameplayPropertyDef");
        if record.guid().to_string() == expected {
            assert_eq!(
                record.type_name()?,
                "CraftingGameplayPropertyDef",
                "actual property source family"
            );
            let tree = record.tree()?;
            assert_eq!(
                tree.root().field("propertyName"),
                Some(&Raw::Locale(key)),
                "actual authored Locale key"
            );
            if owner.replace(record.index()).is_some() {
                return Err("the existing property fixture has one actual physical owner".into());
            }
        }
    }
    Ok(OriginalProperty {
        index: owner.ok_or("original property owner")?,
        guid: expected,
        source_count,
    })
}

pub(crate) fn assert_armour(built: &BuiltCatalog, original: &OriginalProperty) -> TestResult {
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        original.source_count, 1,
        "the original fixture has one property"
    );
    assert_eq!(
        consumer::rows(&catalog, KIND)?.len(),
        1,
        "the complete canonical table retains the original single property"
    );
    let row = row(&catalog, original)?;
    for (field, expected) in [
        ("className", "GPP_Armor_DamageMitigation"),
        (
            "recordPath",
            "libs/foundry/records/crafting/craftedproperties/gpp_armor_damagemitigation",
        ),
    ] {
        assert_eq!(
            consumer::field(row, KIND, field)?.as_str(),
            Some(expected),
            "original expected {field}"
        );
    }
    let name = name(&catalog, row)?;
    assert_eq!(
        consumer::present(name.get("key").ok_or("name key")?)?,
        &json!("@StatName_GPP_Armor"),
        "the original Armour key"
    );
    text(&catalog, name, original, Some("Armour"))
}

pub(crate) fn assert_placeholder(built: &BuiltCatalog, original: &OriginalProperty) -> TestResult {
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = row(&catalog, original)?;
    assert_eq!(
        consumer::field(row, KIND, "className")?.as_str(),
        Some("GPP_Placeholder"),
        "the original placeholder class"
    );
    let name = name(&catalog, row)?;
    assert_eq!(
        consumer::present(name.get("key").ok_or("name key")?)?,
        &json!("@LOC_PLACEHOLDER"),
        "the placeholder retains its authored key"
    );
    text(&catalog, name, original, None)
}

fn row<'catalog>(
    catalog: &'catalog Value,
    original: &OriginalProperty,
) -> TestResult<&'catalog Value> {
    let rows = consumer::rows(catalog, KIND)?;
    for row in rows {
        if consumer::identity(row, KIND)? == original.guid {
            assert_eq!(
                consumer::field(row, KIND, "recordIndex")?,
                &json!(original.index),
                "the original physical owner supplies the row"
            );
            let key = row.get("propertyName").ok_or("property Locale reference")?;
            assert_eq!(
                key.get("containsEncryptedSource"),
                Some(&json!(true)),
                "the fixture's original DCB payload is encrypted"
            );
            assert_eq!(
                key.get("sourceSetComplete"),
                Some(&json!(true)),
                "the actual original key contribution is captured"
            );
            assert_eq!(
                origins::uses(catalog, key)?,
                BTreeSet::from([Use::new(
                    Domain::Body,
                    u64::from(original.index),
                    "propertyName",
                    "payload"
                ),]),
                "the actual property reference keeps its original Locale payload"
            );
            return Ok(row);
        }
    }
    Err("the original property GUID is retained in the canonical table".into())
}

fn name<'catalog>(catalog: &'catalog Value, row: &Value) -> TestResult<&'catalog Value> {
    let reference = consumer::field(row, KIND, "propertyName")?
        .as_str()
        .ok_or("public name reference")?;
    let rows = catalog
        .pointer("/texts/publicNames")
        .and_then(Value::as_array)
        .ok_or("public text rows")?;
    let mut found = None;
    for text in rows {
        if consumer::present(text.get("id").ok_or("text identifier")?)?
            .as_str()
            .ok_or("text identifier string")?
            == reference
            && found.replace(text).is_some()
        {
            return Err("a property text reference resolves to exactly one canonical name".into());
        }
    }
    found.ok_or_else(|| "the actual property reference resolves to its public text row".into())
}

fn text(
    catalog: &Value,
    name: &Value,
    original: &OriginalProperty,
    expected: Option<&str>,
) -> TestResult {
    let translations = consumer::present(name.get("translations").ok_or("translations")?)?
        .as_object()
        .ok_or("available language map")?;
    assert_eq!(
        translations
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["en"]),
        "the actual fixture writes only English; the old German null is a missing language file"
    );
    let english = forms::forms(name, "en")?;
    let lookups = if let Some(expected) = expected {
        forms::present(english, "sourceText", expected)?;
        forms::present(english, "displayText", expected)?;
        [Lookup::DecodedSource, Lookup::Display]
    } else {
        forms::absent(english)?;
        [Lookup::Placeholder, Lookup::Placeholder]
    };
    assert_origins(catalog, english, original, lookups)
}

fn assert_origins(
    catalog: &Value,
    english: &Value,
    original: &OriginalProperty,
    lookups: [Lookup; 2],
) -> TestResult {
    for (field, lookup) in ["sourceText", "displayText"].into_iter().zip(lookups) {
        let carrier = english.get(field).ok_or("direct text form")?;
        assert_eq!(
            carrier.get("containsEncryptedSource"),
            Some(&json!(true)),
            "each {field} retains the actual encrypted authored-key contribution"
        );
        assert_eq!(
            text_origins::actual(catalog, carrier, Language::English)?,
            text_origins::expected(&[(u64::from(original.index), "propertyName")], lookup),
            "the actual BOM-decoding/display or suppressed-lookup contribution of {field}"
        );
    }
    Ok(())
}
