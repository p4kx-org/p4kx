// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::TestResult;
use crate::health_origin_assertions as proof;

pub(super) fn original_values(
    catalog: &Value,
    projection: &Value,
    definition: &Value,
) -> TestResult {
    header(projection, definition);
    attachment(catalog, projection)?;
    physics(projection, definition)?;
    classifications(projection);
    Ok(())
}

fn header(projection: &Value, definition: &Value) {
    for field in [
        "guid",
        "recordName",
        "className",
        "recordPath",
        "recordIndex",
        "sourceType",
        "fileName",
        "metadataTag",
    ] {
        assert_eq!(
            projection.get(field),
            definition.get(field),
            "the original {field} cell is shared exactly"
        );
    }
    assert_eq!(
        projection.pointer("/guid/value"),
        Some(&json!("30000000-0000-4000-8000-000000000001")),
        "original GUID"
    );
    assert_eq!(
        projection.pointer("/className/value"),
        Some(&json!("POWR_ACAS_S01")),
        "original class name"
    );
    assert_eq!(
        projection.pointer("/recordPath/value"),
        Some(&json!(
            "libs/foundry/records/entities/scitem/ships/powerplant/powr_acas_s01"
        )),
        "original path"
    );
}
fn attachment(catalog: &Value, projection: &Value) -> TestResult {
    let attachment = super::selected_attachment(catalog, projection)?;
    for (field, value) in [
        ("itemType", json!("PowerPlant")),
        ("itemSubtype", json!("Power")),
        ("size", json!({"type":"int32", "value":1})),
        ("grade", json!({"type":"int32", "value":3})),
        ("tags", json!(" flightReady  $Small ")),
    ] {
        assert_eq!(
            attachment.pointer(&format!("/{field}/value")),
            Some(&value),
            "exact original {field}"
        );
    }
    assert_eq!(
        attachment.pointer("/manufacturer/value/value/guid/value"),
        Some(&json!("10000000-0000-4000-8000-000000000001")),
        "original manufacturer Reference"
    );
    Ok(())
}

fn classifications(projection: &Value) {
    assert_eq!(
        projection.pointer("/itemClass/value"),
        Some(&json!("Military")),
        "the English classification"
    );
    assert_eq!(
        projection.pointer("/playability/value"),
        Some(&Value::Null),
        "unclassified playability"
    );
    for field in [
        "type",
        "subType",
        "size",
        "grade",
        "mass",
        "manufacturerGuid",
        "tags",
    ] {
        assert!(
            projection.get(field).is_none(),
            "{field} is not duplicated in the root projection"
        );
    }
}

fn physics(projection: &Value, definition: &Value) -> TestResult {
    let slot = projection
        .pointer("/selection/physicsComponentOrdinal/value")
        .ok_or("selected physics slot")?;
    let physics = proof::rows(definition, "/components/value")?
        .iter()
        .find(|row| row.pointer("/componentOrdinal/value") == Some(slot))
        .ok_or("actual original physics")?;
    assert_eq!(
        physics.pointer("/value/value/mass/value"),
        Some(&json!({"rawBits":format!("{:08x}",125.5_f32.to_bits()), "finite":125.5})),
        "exact original mass bits and finite value"
    );
    Ok(())
}
