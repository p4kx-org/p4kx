// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Canonical owned resource properties retain decoder failures and every IEEE payload bit.

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog, catalog_schema};
use p4kx_extraction::{DataForge, Value as ForgeValue, ValueError};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::catalog_crafting_support;
use crate::health_origin_assertions;
use crate::resource_1069_fixture as fixture;
use crate::resource_origin_fixture::{self as origin, TestResult};

#[test]
fn an_unreadable_owned_cell_keeps_its_resource_and_readable_neighbors() -> TestResult {
    let database = fixture::unreadable_property();
    assert_decoder_failure_is_field_local(&database)?;
    let Err(error) = fixture::build(&database, CatalogPublicationMode::Normal) else {
        return Err("an unreadable owned field must block normal publication".into());
    };
    assert!(matches!(
        error.downcast_ref::<CatalogError>(),
        Some(CatalogError::IncompleteExport(_))
    ));
    let (_, catalog) = fixture::build(&database, CatalogPublicationMode::DiagnosticPartial)?;
    let resource = fixture::row(&catalog)?;
    assert_eq!(
        resource["guid"]["value"],
        catalog_crafting_support::text(fixture::RESOURCE)
    );
    let property = resource
        .pointer("/properties/value/0/value")
        .ok_or("owned property")?;
    assert_eq!(property["sourceType"]["value"], "ResourceTypeVolatility");
    assert_eq!(property["name"]["value"], "Readable neighbor");
    let unreadable = property
        .pointer("/payload/value/volatility")
        .ok_or("unreadable value")?;
    assert_eq!(unreadable["state"], "unreadable");
    assert_eq!(unreadable["value"], Value::Null);
    assert_eq!(unreadable["containsEncryptedSource"], Value::Null);
    assert_eq!(unreadable["sourceSetComplete"], false);
    assert_eq!(
        unreadable["sourceUsesRefs"],
        json!([]),
        "attempts are not contributors"
    );
    let neighbor = property
        .pointer("/payload/value/healthDecayPerSecond")
        .ok_or("readable neighbor")?;
    assert_eq!(neighbor["state"], "present");
    assert_eq!(neighbor["value"]["rawBits"], "3fa00000");
    assert_eq!(neighbor["value"]["finite"], 1.25);
    assert_owned_origin(&catalog, neighbor, "properties[0].healthDecayPerSecond")?;
    assert_unreadable_evidence(&catalog)?;
    assert_eq!(catalog["exportHealth"]["status"], "incomplete");
    Ok(())
}

fn assert_decoder_failure_is_field_local(database: &DataForgeBuilder) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("the owning ResourceType")?.tree()?;
    let Some(ForgeValue::Array(properties)) = tree.root().field("properties") else {
        return Err("the owning property array must remain readable".into());
    };
    let Some(ForgeValue::Struct(property)) = properties.first() else {
        return Err("the concrete owned property must remain readable".into());
    };
    assert!(
        matches!(
            property.field("volatility"),
            Some(ForgeValue::Error(ValueError::InvalidString {
                offset: u32::MAX,
                ..
            }))
        ),
        "the parser produced an actual field-local decoding error"
    );
    assert!(
        matches!(
            property.field("name"),
            Some(ForgeValue::String("Readable neighbor"))
        ),
        "the inherited neighbor is readable"
    );
    assert!(
        matches!(
            property.field("healthDecayPerSecond"),
            Some(ForgeValue::Single(value)) if value.to_bits() == 0x3fa0_0000
        ),
        "the following scalar remains at its authored position"
    );
    Ok(())
}

fn assert_unreadable_evidence(catalog: &Value) -> TestResult {
    let uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source-use inventory")?;
    assert!(
        uses.iter().any(|contribution| {
            contribution.get("role").and_then(Value::as_str) == Some("attempted")
                && contribution
                    .pointer("/location/recordIndex")
                    .and_then(Value::as_u64)
                    == Some(0)
                && contribution
                    .pointer("/location/field")
                    .and_then(Value::as_str)
                    == Some("properties[0].volatility")
        }),
        "the failed cell has a separate actual attempt"
    );
    let diagnostics = catalog
        .pointer("/exportHealth/diagnostics")
        .and_then(Value::as_array)
        .ok_or("field diagnostics")?;
    for stage in ["sourceField", "projection"] {
        assert!(
            exact_unreadable_example(catalog, diagnostics, stage)?,
            "the exact owner and field are retained at {stage}"
        );
    }
    assert!(
        diagnostics.iter().all(|diagnostic| {
            diagnostic.get("code").and_then(Value::as_str) != Some("sourceReadFailure")
        }),
        "a local cell failure is not a wholly failed Record"
    );
    Ok(())
}

fn exact_unreadable_example(
    catalog: &Value,
    diagnostics: &[Value],
    stage: &str,
) -> TestResult<bool> {
    for diagnostic in diagnostics.iter().filter(|row| {
        row["code"].as_str() == Some("unreadableSource") && row["stage"].as_str() == Some(stage)
    }) {
        let Some(examples) = diagnostic["examples"].as_array() else {
            continue;
        };
        for example in examples {
            if example
                .pointer("/recordIndex/value")
                .and_then(Value::as_u64)
                == Some(0)
                && health_origin_assertions::example_text(catalog, example, "field")?
                    .and_then(Value::as_str)
                    == Some("properties[0].volatility")
                && health_origin_assertions::example_text(catalog, example, "sourceType")?
                    .and_then(Value::as_str)
                    == Some("ResourceTypeVolatility")
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[test]
fn nonfinite_owned_values_keep_exact_bits_origin_order_and_repeatable_output() -> TestResult {
    let (_directory, archive) = origin::open(fixture::nonfinite_properties().build(), false)?;
    let options = origin::options();
    let built = build_catalog(&archive, &options)?;
    let repeated = build_catalog(&archive, &options)?;
    assert_eq!(
        built.json(),
        repeated.json(),
        "parallel preparation preserves source identities"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    let properties = fixture::row(&catalog)?
        .pointer("/properties/value")
        .and_then(Value::as_array)
        .ok_or("ordered property values")?;
    assert_eq!(
        properties.len(),
        3,
        "duplicates and null positions remain authored"
    );
    assert_eq!(
        properties
            .first()
            .and_then(|property| property.pointer("/value/name/value")),
        Some(&json!("Repeated"))
    );
    assert_eq!(
        properties.get(1).and_then(|property| property.get("state")),
        Some(&json!("null"))
    );
    assert_eq!(
        properties.get(1).and_then(|property| property.get("value")),
        Some(&Value::Null)
    );
    assert_eq!(
        properties
            .get(2)
            .and_then(|property| property.pointer("/value/name/value")),
        Some(&json!("Repeated"))
    );
    assert_nonfinite_fields(&catalog)?;
    assert_eq!(
        catalog["exportHealth"]["status"], "complete",
        "preserved legal IEEE values do not make source health fail"
    );
    let schema = catalog_schema();
    assert!(
        jsonschema::validator_for(&schema)?.is_valid(&catalog),
        "present IEEE values with null finite projections satisfy the real v3 schema"
    );
    Ok(())
}

fn assert_nonfinite_fields(catalog: &Value) -> TestResult {
    let resource = fixture::row(catalog)?;
    for (path, bits, field) in [
        (
            "/properties/value/0/value/payload/value/volatility",
            "7fc01234",
            "properties[0].volatility",
        ),
        (
            "/properties/value/0/value/payload/value/healthDecayPerSecond",
            "7f800000",
            "properties[0].healthDecayPerSecond",
        ),
        (
            "/properties/value/2/value/payload/value/volatility",
            "ff800000",
            "properties[2].volatility",
        ),
        (
            "/properties/value/2/value/payload/value/healthDecayPerSecond",
            "ffc0abcd",
            "properties[2].healthDecayPerSecond",
        ),
    ] {
        let value = resource.pointer(path).ok_or("present nonfinite field")?;
        assert_eq!(
            value["state"], "present",
            "legal source bits remain a present fact"
        );
        assert_eq!(
            value.pointer("/value/rawBits").and_then(Value::as_str),
            Some(bits),
            "sign and NaN payload bits survive"
        );
        assert_eq!(
            value.pointer("/value/finite"),
            Some(&Value::Null),
            "decimal projection is separate"
        );
        assert_owned_origin(catalog, value, field)?;
    }
    Ok(())
}

fn assert_owned_origin(catalog: &Value, value: &Value, field: &str) -> TestResult {
    assert_eq!(
        value["containsEncryptedSource"], false,
        "the retained cell has known plain origin"
    );
    assert_eq!(
        value["sourceSetComplete"], true,
        "the retained cell has complete actual contributor evidence"
    );
    let references = value
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("immediate field source references")?;
    assert_eq!(
        references.len(),
        1,
        "the exact retained cell contributes once"
    );
    let id = references.first().ok_or("field source-use reference")?;
    let uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source-use inventory")?;
    let contribution = uses
        .iter()
        .find(|contribution| contribution.get("id") == Some(id))
        .ok_or("the immediate field reference must resolve")?;
    assert_eq!(
        contribution["scope"], "payload",
        "the inspected source contributes retained payload"
    );
    assert_eq!(
        contribution["role"], "payload",
        "attempted reads cannot stand in for this retained value"
    );
    assert_eq!(
        contribution["location"],
        json!({
            "type": "dataForge", "recordIndex": 0, "field": field
        }),
        "origin names the actual owner and authored list position"
    );
    assert_physical_source(catalog, contribution)
}

fn assert_physical_source(catalog: &Value, contribution: &Value) -> TestResult {
    let source_id = contribution
        .get("source")
        .ok_or("physical source reference")?;
    let sources = catalog
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical source inventory")?;
    let source = sources
        .iter()
        .find(|source| source.get("id") == Some(source_id))
        .ok_or("the actual physical source must resolve")?;
    assert_eq!(
        source["entryIndex"], 0,
        "the value refers to the actual physical DataForge Entry"
    );
    assert_eq!(
        source["storedEncryption"], "unencrypted",
        "an unrelated encrypted language does not taint this source cell"
    );
    Ok(())
}
