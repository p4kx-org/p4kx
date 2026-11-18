// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::{catalog_from, fixture, loadout};
use crate::commodity_v3_fixture::TestResult;
use crate::health_origin_assertions as proof;

#[test]
fn inline_vehicle_port_override_retains_exact_value_and_selector_sites() -> TestResult {
    check(
        fixture::GraphSource::Elem,
        [
            ("portTags", json!(["$Elem", "Cargo"]), "value", "conversion"),
            ("minSize", json!(4), "value", "conversion"),
            ("displayNameKey", json!("@inline_port"), "value", "payload"),
        ],
        &["name", "idRef", "id"],
    )
}

#[test]
fn by_id_vehicle_patch_retains_donor_attribute_sites() -> TestResult {
    check(
        fixture::GraphSource::Patch,
        [
            (
                "requiredTags",
                json!(["$Patch", "Cargo"]),
                "requiredTags",
                "conversion",
            ),
            ("maxSize", json!(7), "maxSize", "conversion"),
            ("displayNameKey", json!(""), "display_name", "payload"),
        ],
        &["id"],
    )
}

#[test]
fn an_inline_display_override_without_value_carries_its_checked_absence() -> TestResult {
    let (_folder, archive) =
        fixture::graph_archive_for(fixture::GraphSource::ElemWithoutDisplayValue)?;
    let catalog = catalog_from(&archive)?;
    let cell = proof::cell(effective_port(&catalog)?, "displayNameKey", "absent")?;
    let uses = proof::uses(&catalog, cell)?;
    assert!(
        uses.iter().any(|usage| {
            usage["location"]["namespace"] == "textXmlElement"
                && usage["location"]["field"] == "value"
                && usage["role"] == "selector"
        }),
        "omission is tied to the Elem that selected the empty fallback",
    );
    assert!(uses.iter().all(|usage| usage["role"] != "payload"));
    proof::closure(&catalog)
}

fn check(
    source: fixture::GraphSource,
    fields: [(&str, Value, &str, &str); 3],
    selectors: &[&str],
) -> TestResult {
    let (_folder, archive) = fixture::graph_archive_for(source)?;
    let catalog = catalog_from(&archive)?;
    let port = effective_port(&catalog)?;
    for (field, expected, input, role) in fields {
        let cell = proof::cell(port, field, "present")?;
        assert_eq!(
            cell.get("value"),
            Some(&expected),
            "the override sets {field}"
        );
        let uses = proof::uses(&catalog, cell)?;
        assert!(
            contains(&uses, input, role),
            "{field} cites the applied donor value",
        );
        for &selector in selectors {
            assert!(
                contains(&uses, selector, "selector"),
                "{field} cites selector {selector}",
            );
        }
    }
    proof::closure(&catalog)
}

fn effective_port(catalog: &Value) -> TestResult<&Value> {
    let (_, ship) = loadout(catalog, fixture::SHIP)?;
    proof::rows(ship, "/ports")?
        .iter()
        .find(|row| row.pointer("/name/value").and_then(Value::as_str) == Some("vehicle_only"))
        .ok_or_else(|| "the effective XML port".into())
}

fn contains(uses: &[&Value], field: &str, role: &str) -> bool {
    uses.iter().any(|usage| {
        usage.pointer("/location/namespace").and_then(Value::as_str) == Some("textXmlAttribute")
            && usage.pointer("/location/field").and_then(Value::as_str) == Some(field)
            && usage.get("role").and_then(Value::as_str) == Some(role)
    })
}
