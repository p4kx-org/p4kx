// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::text_routes::{
    TextDomain, TextPresence, check_nullable_reference, check_string, check_text_route,
};
use super::{Inventory, Result, family_fields, present_value, released_schema, schema_at};

const DEFINITION: &str = "/definitions/commodities/rows/[]";
const PRESENTATION: &str = "/projections/commodityPresentation/rows/[]";

#[test]
fn released_v2_commodity_fields_have_explicit_dispositions() -> Result {
    let legacy = released_schema()?;
    let inventory = Inventory::collect(&legacy)?;
    let released = family_fields(&inventory, "commodities")?;
    let current = p4kx_export::catalog_schema();

    assert!(current["properties"].get("commodities").is_none());
    let mut classified = BTreeSet::new();
    verify_definition(&current, &mut classified)?;
    verify_presentation(&current, &mut classified)?;
    assert_eq!(
        released,
        classified.iter().map(String::as_str).collect(),
        "released Commodity migration coverage"
    );
    Ok(())
}

fn verify_definition(schema: &Value, classified: &mut BTreeSet<String>) -> Result {
    for field in ["guid", "className", "recordPath"] {
        classified.insert(format!("/{field}"));
        check_string(schema, &format!("{DEFINITION}/{field}/value"))?;
    }

    let definition = schema_at(schema, DEFINITION)?;
    let presentation = schema_at(schema, PRESENTATION)?;
    let identity = definition
        .pointer("/properties/guid/$ref")
        .and_then(Value::as_str);
    assert_eq!(
        identity,
        Some("#/$defs/SourcedValueOfGuid"),
        "Commodity definition identity"
    );
    assert_eq!(
        presentation
            .pointer("/properties/commodityGuid/$ref")
            .and_then(Value::as_str),
        identity,
        "join by Commodity GUID"
    );
    Ok(())
}

fn verify_presentation(schema: &Value, classified: &mut BTreeSet<String>) -> Result {
    for field in ["subtypeGuid", "refinedSubtypeGuid", "typeGuid"] {
        classified.insert(format!("/{field}"));
        let wrapper = schema_at(schema, &format!("{PRESENTATION}/{field}"))?;
        check_nullable_reference(wrapper, "Guid")?;
    }

    let density = "densityGramsPerCubicCentimetre";
    classified.insert(format!("/{density}"));
    let wrapper = schema_at(schema, &format!("{PRESENTATION}/{density}"))?;
    let payload = present_value(wrapper).ok_or("Commodity density payload")?;
    assert_eq!(
        payload.get("type"),
        Some(&json!(["number", "null"])),
        "present density is numeric or null"
    );
    assert_eq!(
        payload.get("format").and_then(Value::as_str),
        Some("float"),
        "density retains its Single representation"
    );

    for (field, domain) in [
        ("name", TextDomain::PublicName),
        ("description", TextDomain::GameText),
        ("displayType", TextDomain::GameText),
    ] {
        for suffix in ["", "/key", "/en", "/de"] {
            classified.insert(format!("/{field}{suffix}"));
        }
        check_text_route(schema, PRESENTATION, field, domain, TextPresence::Nullable)?;
    }
    Ok(())
}
