// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Direct product-wire and Basetool consumer checks for the complete resource family.

use std::collections::BTreeSet;
use std::fs;

use p4kx_export::{
    CatalogFile, Compression, build_catalog, build_catalog_from, write_catalog_file,
    write_catalog_file_from,
};
use p4kx_extraction::{DataForgeEntry, Language};
use serde::Deserialize;
use serde_json::Value;

use crate::resource_origin_fixture::{self as fixture, TestResult};
use crate::resource_quality_schema_fixture::OverrideEntryTarget;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportedValue<T> {
    state: String,
    value: Option<T>,
    contains_encrypted_source: Option<bool>,
    source_uses_refs: Vec<String>,
    source_set_complete: bool,
}

impl<T> ImportedValue<T> {
    fn public_value(&self) -> Option<&T> {
        (self.state == "present"
            && self.source_set_complete
            && self.contains_encrypted_source == Some(false))
        .then_some(self.value.as_ref())
        .flatten()
    }
}

fn row(catalog: &Value) -> TestResult<&Value> {
    catalog
        .pointer("/definitions/resourceTypes/rows/0")
        .ok_or_else(|| "resource row missing".into())
}

fn text_row<'a>(catalog: &'a Value, domain: &str, key: &str) -> TestResult<&'a Value> {
    catalog
        .get("texts")
        .and_then(|texts| texts.get(domain))
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.pointer("/key/value").and_then(Value::as_str) == Some(key))
        })
        .ok_or_else(|| "canonical text row missing".into())
}

#[test]
fn basetool_classifies_each_value_without_reconstructing_provenance() -> TestResult {
    let (_directory, archive) = fixture::open(fixture::database(2.5).build(), false)?;
    let built = build_catalog(&archive, &fixture::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert!(
        catalog.get("resourceTypes").is_none(),
        "no permanent flat ResourceType duplicate"
    );
    assert_eq!(
        catalog["definitions"]["resourceTypes"]["availability"],
        "available"
    );
    let resource = row(&catalog)?;
    let guid: ImportedValue<String> = serde_json::from_value(resource["guid"].clone())?;
    assert!(
        guid.public_value().is_some(),
        "the DCB payload is unencrypted"
    );
    assert!(
        !guid.source_uses_refs.is_empty(),
        "immediate origin refers to actual uses"
    );
    let names = text_row(&catalog, "publicNames", "@fixture_name")?;
    let translations = &names["translations"]["value"];
    let english: ImportedValue<String> = serde_json::from_value(translations["en"].clone())?;
    let german: ImportedValue<String> = serde_json::from_value(translations["de"].clone())?;
    let french: ImportedValue<String> = serde_json::from_value(translations["fr"].clone())?;
    assert_eq!(english.value.as_deref(), Some("English winner"));
    assert_eq!(english.contains_encrypted_source, Some(true));
    assert!(
        english.public_value().is_none(),
        "encrypted language contribution is not public"
    );
    assert!(
        german.public_value().is_some(),
        "this language uses only known unencrypted input"
    );
    assert_eq!(french.contains_encrypted_source, None);
    assert!(
        french.public_value().is_none(),
        "unknown origin cannot be promoted by read success"
    );
    assert!(
        !english.source_uses_refs.is_empty() && !german.source_uses_refs.is_empty(),
        "each localized value carries its own uses"
    );
    Ok(())
}

#[test]
fn every_nested_resource_value_has_the_immediate_shared_carrier() -> TestResult {
    let database = crate::resource_fixture::database(OverrideEntryTarget::Concrete).build();
    let (_directory, archive) = fixture::open(database, false)?;
    let built = build_catalog(&archive, &fixture::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let rows = catalog["definitions"]["resourceTypes"]["rows"]
        .as_array()
        .ok_or("typed rows")?;
    assert_eq!(rows.len(), 3);
    let mut visited = 0;
    for row in rows {
        assert_values(row, &mut visited)?;
    }
    assert!(
        visited > 100,
        "identities, links, nested properties and quality data are all covered"
    );
    check_reference_closure(&catalog)?;
    Ok(())
}

#[test]
fn raw_ieee_values_are_present_and_zero_false_empty_null_absent_stay_distinct() -> TestResult {
    let (_directory, archive) = fixture::open(
        fixture::database(f32::from_bits(0x7fc0_1234)).build(),
        false,
    )?;
    let built = build_catalog(&archive, &fixture::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let resource = row(&catalog)?;
    assert_eq!(catalog["exportHealth"]["status"], "complete");
    assert_eq!(
        resource["density"]["value"]["payload"]["value"]["molarMass"]["state"],
        "present"
    );
    assert_eq!(
        resource["density"]["value"]["payload"]["value"]["molarMass"]["value"]["rawBits"],
        "7fc01234"
    );
    assert!(
        resource["density"]["value"]["payload"]["value"]["molarMass"]["value"]["finite"].is_null(),
        "NaN has no decimal projection"
    );
    assert_eq!(resource["validateDefaultCargoBox"]["value"], false);
    assert_eq!(resource["defaultThumbnailPath"]["value"], "");
    assert_eq!(resource["defaultThumbnailPathSVG"]["state"], "absent");
    assert_eq!(resource["refinedVersion"]["state"], "null");
    assert_eq!(resource["properties"]["value"][1]["state"], "null");
    assert_eq!(
        resource["properties"]["value"][0]["value"]["payload"]["value"]["volatility"]["value"]["rawBits"],
        "00000000"
    );
    Ok(())
}

#[test]
fn the_quality_distribution_retains_its_nonfinite_token_and_direct_plain_origin() -> TestResult {
    let (_directory, archive) = fixture::open(fixture::unported().build(), false)?;
    let built = build_catalog(&archive, &fixture::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = crate::quality_v3_support::rows(&catalog, crate::quality_v3_support::DISTRIBUTIONS)?
        .first()
        .ok_or("quality definition")?;
    let mean = crate::quality_v3_support::at(
        row,
        "/qualityDistribution/value/payload/value/value/value/mean",
    )?;
    assert_eq!(
        crate::quality_v3_support::at(mean, "/value")?,
        &serde_json::json!({"type":"f32", "rawBits":format!("{:08x}", f32::NAN.to_bits()), "finite":null})
    );
    crate::quality_v3_support::marker(mean, Some(false))?;
    assert_eq!(
        crate::quality_v3_support::at(mean, "/sourceSetComplete")?,
        &serde_json::json!(true)
    );
    assert_eq!(
        crate::quality_v3_support::at(&catalog, "/exportHealth/status")?,
        &serde_json::json!("complete")
    );
    Ok(())
}

#[test]
fn every_available_supported_language_uses_its_actual_winning_entry() -> TestResult {
    let (_directory, archive) = fixture::open(fixture::database(1.0).build(), true)?;
    let built = build_catalog(&archive, &fixture::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let text = text_row(&catalog, "publicNames", "@fixture_name")?;
    let languages = text["translations"]["value"]
        .as_object()
        .ok_or("language map")?;
    assert_eq!(languages.len(), Language::ALL.len());
    for language in Language::ALL {
        assert!(
            languages.contains_key(language.code()),
            "available language is retained"
        );
    }
    let sources = catalog["provenance"]["sources"]
        .as_array()
        .ok_or("physical sources")?;
    let english = sources
        .iter()
        .find(|source| source["path"]["value"] == Language::English.global_ini_path())
        .ok_or("winning English source")?;
    assert_eq!(english["entryIndex"], 1);
    assert_eq!(english["storedEncryption"], "encrypted");
    assert_eq!(
        english["path"]["containsEncryptedSource"], false,
        "root directory metadata is not tainted by the leaf payload"
    );
    Ok(())
}

#[test]
fn cli_and_gui_file_adapters_write_the_same_prepared_plain_and_compressed_values() -> TestResult {
    let (directory, archive) = fixture::open(fixture::database(-0.0).build(), false)?;
    let baseline = build_catalog(&archive, &fixture::options())?;
    let plain = directory.path().join("catalog.json");
    let (_, file) = write_catalog_file(&archive, &fixture::options(), Compression::None, || {
        CatalogFile::create(&plain)
    })?;
    file.commit()?;
    let loaded = DataForgeEntry::read(&archive)?;
    let packed = directory.path().join("catalog.json.zst");
    let (_, file) = write_catalog_file_from(
        &archive,
        &loaded,
        &fixture::options(),
        Compression::Zstd { level: 1 },
        || CatalogFile::create(&packed),
    )?;
    file.commit()?;
    assert_eq!(fs::read(&plain)?, baseline.json());
    let unpacked = zstd::stream::decode_all(fs::File::open(&packed)?)?;
    assert_eq!(
        unpacked,
        baseline.json(),
        "sizing/compression does not reproject sources"
    );
    Ok(())
}

#[test]
fn standalone_dcb_origin_is_not_inferred_from_the_localization_archive() -> TestResult {
    let (directory, archive) = fixture::open(fixture::database(3.0).build(), false)?;
    let path = directory.path().join("external.dcb");
    fs::write(&path, fixture::database(3.0).build())?;
    let external = DataForgeEntry::read_file(&path)?;
    let built = build_catalog_from(&archive, &external, &fixture::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert!(
        row(&catalog)?["guid"]["containsEncryptedSource"].is_null(),
        "file P4K lineage is unknown"
    );
    let german =
        &text_row(&catalog, "publicNames", "@fixture_name")?["translations"]["value"]["de"];
    assert!(
        german["containsEncryptedSource"].is_null(),
        "the actual unknown DCB key selector contributes"
    );
    Ok(())
}

fn assert_values(value: &Value, visited: &mut usize) -> TestResult {
    match value {
        Value::Object(object) if object.contains_key("state") => {
            for key in [
                "state",
                "value",
                "containsEncryptedSource",
                "sourceUsesRefs",
                "sourceSetComplete",
            ] {
                assert!(
                    object.contains_key(key),
                    "every emitted nullable carrier key is present: {key}"
                );
            }
            *visited += 1;
            assert_values(&object["value"], visited)?;
        }
        Value::Object(object) if object.contains_key("rawBits") => {
            assert!(
                object.contains_key("finite"),
                "IEEE value has its explicit nullable decimal"
            );
        }
        Value::Object(object)
            if object
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.starts_with("int") || kind.starts_with("uint")) => {}
        Value::Object(object) => {
            for (name, child) in object {
                if name == "type" || name == "status" {
                    continue;
                }
                assert!(
                    child.get("state").is_some(),
                    "source-backed nested field has a carrier: {name}"
                );
                assert_values(child, visited)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                assert!(
                    value.get("state").is_some(),
                    "each source-backed array element has a carrier"
                );
                assert_values(value, visited)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_reference_closure(catalog: &Value) -> TestResult {
    fn walk(value: &Value, ids: &BTreeSet<&str>) -> TestResult {
        if let Some(references) = value.get("sourceUsesRefs").and_then(Value::as_array) {
            for reference in references {
                assert!(
                    ids.contains(reference.as_str().ok_or("use id")?),
                    "every actual use reference resolves"
                );
            }
        }
        match value {
            Value::Object(object) => {
                for value in object.values() {
                    walk(value, ids)?;
                }
            }
            Value::Array(values) => {
                for value in values {
                    walk(value, ids)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("use inventory")?;
    let ids: BTreeSet<_> = uses
        .iter()
        .filter_map(|value| value.get("id").and_then(Value::as_str))
        .collect();
    walk(catalog, &ids)
}
