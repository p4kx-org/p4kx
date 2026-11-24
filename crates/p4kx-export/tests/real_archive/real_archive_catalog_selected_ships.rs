// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::de::{DeserializeSeed, Deserializer, Error, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy)]
pub(super) struct SelectedShipIdentities(pub(super) fn(&str) -> bool);

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShipIdentity {
    class_name: Value,
    guid: Value,
    record_index: Value,
    playability: Value,
}

impl<'de> DeserializeSeed<'de> for SelectedShipIdentities {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for SelectedShipIdentities {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Catalog Ship array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut selected = Vec::new();
        while let Some(ship) = sequence.next_element::<ShipIdentity>()? {
            if ship
                .class_name
                .get("value")
                .and_then(Value::as_str)
                .is_some_and(self.0)
            {
                selected.push(serde_json::to_value(ship).map_err(A::Error::custom)?);
            }
        }
        Ok(Value::Array(selected))
    }
}

#[test]
fn ship_identity_capture_keeps_only_selected_identity_fields_and_preserves_aliases()
-> super::TestResult {
    use serde_json::json;

    let picked = |class: &str| class.eq_ignore_ascii_case("Selected");
    let input = r#"[{"className":{"value":"Other"},"guid":{},"recordIndex":{},"playability":{},"unrelated":[{}]},
        {"className":{"value":"Selected"},"guid":{"value":"match"},"recordIndex":{"value":1},"playability":{"value":"player"},"unrelated":[1]},
        {"className":{"value":"selected"},"guid":{"value":"match"},"recordIndex":{"value":2},"playability":{"value":"ai"}}]"#;
    let mut decoder = serde_json::Deserializer::from_str(input);
    let result = SelectedShipIdentities(picked).deserialize(&mut decoder)?;
    decoder.end()?;
    let rows = result.as_array().ok_or("projected Ship rows")?;
    assert_eq!(rows.len(), 2, "both physical aliases must reach the caller");
    assert_eq!(result.pointer("/0/recordIndex/value"), Some(&json!(1)));
    assert_eq!(result.pointer("/1/recordIndex/value"), Some(&json!(2)));
    assert_eq!(result.pointer("/0/unrelated"), None);
    Ok(())
}
