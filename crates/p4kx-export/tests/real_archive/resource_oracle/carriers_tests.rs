// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use serde_json::{Value, json};

use super::{Carriers, Uses, contributing_roles};
use crate::real_archive_resource_oracles::Result;

fn uses(rows: &[Value]) -> Uses<'_> {
    Uses {
        dcb: "synthetic-dcb",
        marker: Value::Null,
        roles: contributing_roles().expect("shared contribution vocabulary"),
        rows: rows
            .iter()
            .map(|row| (row["id"].as_str().expect("synthetic use id"), row))
            .collect::<HashMap<_, _>>(),
    }
}

fn use_row(id: &str, field: &str, owner: u32) -> Value {
    json!({"id": id, "source": "synthetic-dcb", "scope": "payload", "role": "payload",
        "location": {"type": "dataForge", "recordIndex": owner, "field": field}})
}

fn carrier(state: &str, value: &Value, references: &[&str]) -> Value {
    json!({"state": state, "value": value, "sourceUsesRefs": references,
        "containsEncryptedSource": null, "sourceSetComplete": true})
}

#[test]
fn nominal_aliases_do_not_replace_exact_physical_record_ownership() -> Result<()> {
    let rows = [use_row("first", "$guid", 3), use_row("second", "$guid", 7)];
    let inventory = uses(&rows);
    assert_eq!(
        inventory.record_index(&json!({"guid":
        carrier("present", &json!("synthetic-guid"), &["first"])}))?,
        3,
        "the physical Record index comes from its GUID field origin"
    );
    assert!(
        inventory
            .record_index(&json!({"guid":
        carrier("present", &json!("synthetic-guid"), &["first", "second"])}))
            .is_err(),
        "same-GUID aliases cannot select the first origin"
    );
    assert!(
        inventory
            .record_index(&json!({"guid":
        carrier("present", &json!("synthetic-guid"), &["missing"])}))
            .is_err(),
        "every origin reference must resolve"
    );
    Ok(())
}

#[test]
fn array_positions_retain_their_exact_field_origins() -> Result<()> {
    let rows = [
        use_row("array", "properties", 3),
        use_row("left", "properties[0]", 3),
        use_row("right", "properties[1]", 3),
    ];
    let inventory = uses(&rows);
    let output = json!({"properties": carrier("present", &json!([
        carrier("null", &Value::Null, &["right"]),
        carrier("null", &Value::Null, &["left"]),
    ]), &["array"])});
    let carriers = Carriers::new(&output, &inventory, 3)?;
    let array = carriers.array("properties", 2).expect("synthetic array");
    assert!(
        !carriers.contains(
            "properties[0]",
            array.first().expect("synthetic first position")
        ),
        "reordered origins cannot pass by value equality"
    );
    assert!(
        carriers.contains(
            "properties[0]",
            array.get(1).expect("synthetic second position")
        ),
        "the original physical position remains identifiable"
    );
    assert!(
        carriers.array("properties", 1).is_none(),
        "array length is exact"
    );
    Ok(())
}
