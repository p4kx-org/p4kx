// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};

use super::auto_properties;
use crate::mission_values_fixture::{self as fixture, Owner, TestResult};

#[test]
fn released_v2_property_union_rejects_emitted_v3_auto_spawns_alternatives() -> TestResult {
    let released: Value =
        serde_json::from_str(include_str!("../fixtures/catalog-v0.4.2.schema.json"))?;
    let current = p4kx_export::catalog_schema();
    let released_union = jsonschema::validator_for(&property_union(&released))?;
    let current_union = jsonschema::validator_for(&property_union(&current))?;
    let current_catalog = jsonschema::validator_for(&current)?;
    assert_eq!(
        released.pointer("/properties/schemaVersion/const"),
        Some(&json!(2)),
        "the frozen schema belongs to released v2"
    );
    let old_positive = json!({"type":"object"});
    assert!(
        released_union.is_valid(&old_positive),
        "the isolated old schema resolves and admits a known historical branch"
    );
    assert!(
        !released_union.is_valid(&json!({"type":"autoSpawns"})),
        "changing just the known tag cannot enter the released closed union"
    );

    for owner in [Owner::Generator, Owner::Template, Owner::Broker] {
        let catalog = fixture::build(
            &fixture::database(auto_properties(), owner),
            CatalogPublicationMode::Normal,
        )?;
        assert_eq!(
            catalog.get("schemaVersion"),
            Some(&json!(3)),
            "the writer emits v3 for each physical owner"
        );
        current_catalog
            .validate(&catalog)
            .map_err(|error| format!("actual v3 catalog validation: {error}"))?;
        if matches!(owner, Owner::Generator) {
            for (variable, expected) in [
                ("Base", "base"),
                ("Entity", "entityDeclaration"),
                ("Tags", "tags"),
            ] {
                let property = fixture::generator_value(&catalog, variable)?;
                assert_eq!(
                    property
                        .pointer("/searchOptions/type")
                        .and_then(Value::as_str),
                    Some(expected),
                    "the actual emitted closet-search discriminator"
                );
                current_union
                    .validate(property)
                    .map_err(|error| format!("actual v3 property validation: {error}"))?;
                assert!(
                    !released_union.is_valid(property),
                    "released v2 rejects the actual emitted {expected} payload"
                );
            }
        }
    }
    Ok(())
}

fn property_union(schema: &Value) -> Value {
    json!({"$ref":"#/$defs/PropertyValue", "$defs":schema.get("$defs")})
}
