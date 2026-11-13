// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::marker::PhantomData;

use serde::Deserialize;
use serde::de::{DeserializeSeed, Deserializer, Error as _, MapAccess, Visitor};

use crate::origin_reader_limits::ReadBudget;
use crate::source_use_wire::ReferencesSeed;
use crate::{FieldOrigin, OriginReaderLimits, SourcedValue};

/// Reads one sourced value under an explicit origin-metadata policy.
pub struct SourcedValueSeed<T> {
    limits: OriginReaderLimits,
    value: PhantomData<fn() -> T>,
}

impl<T> SourcedValueSeed<T> {
    /// Uses the given contribution limits for this value.
    #[must_use]
    pub const fn new(limits: OriginReaderLimits) -> Self {
        Self {
            limits,
            value: PhantomData,
        }
    }
}

impl<'de, T: Deserialize<'de>> DeserializeSeed<'de> for SourcedValueSeed<T> {
    type Value = SourcedValue<T>;

    fn deserialize<D: Deserializer<'de>>(self, input: D) -> Result<Self::Value, D::Error> {
        input.deserialize_map(self)
    }
}

impl<'de, T: Deserialize<'de>> Visitor<'de> for SourcedValueSeed<T> {
    type Value = SourcedValue<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a typed sourced value with correlated origin markers")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let budget = ReadBudget::new(self.limits.metadata_bytes);
        budget.charge(size_of::<FieldOrigin>())?;
        let mut state = None;
        let mut value = None;
        let mut encrypted = None;
        let mut references = None;
        let mut complete = None;
        while let Some(field) = map.next_key::<Field>()? {
            match field {
                Field::State => store(&mut state, "state", || map.next_value())?,
                Field::Value => store(&mut value, "value", || map.next_value())?,
                Field::Encrypted => store(&mut encrypted, "containsEncryptedSource", || {
                    map.next_value()
                })?,
                Field::References => store(&mut references, "sourceUsesRefs", || {
                    map.next_value_seed(ReferencesSeed {
                        limits: self.limits,
                        budget: &budget,
                    })
                })?,
                Field::Complete => store(&mut complete, "sourceSetComplete", || map.next_value())?,
            }
        }
        let origin = FieldOrigin::from_wire(
            required(encrypted, "containsEncryptedSource")?,
            required(references, "sourceUsesRefs")?,
            required(complete, "sourceSetComplete")?,
        )
        .map_err(M::Error::custom)?;
        SourcedValue::from_wire(required(state, "state")?, required(value, "value")?, origin)
            .ok_or_else(|| M::Error::custom("source value state does not match its value"))
    }
}

fn store<T, E: serde::de::Error>(
    destination: &mut Option<T>,
    field: &'static str,
    read: impl FnOnce() -> Result<T, E>,
) -> Result<(), E> {
    if destination.is_some() {
        return Err(E::duplicate_field(field));
    }
    *destination = Some(read()?);
    Ok(())
}

fn required<T, E: serde::de::Error>(value: Option<T>, field: &'static str) -> Result<T, E> {
    value.ok_or_else(|| E::missing_field(field))
}

#[derive(Deserialize)]
#[serde(field_identifier)]
enum Field {
    #[serde(rename = "state")]
    State,
    #[serde(rename = "value")]
    Value,
    #[serde(rename = "containsEncryptedSource")]
    Encrypted,
    #[serde(rename = "sourceUsesRefs")]
    References,
    #[serde(rename = "sourceSetComplete")]
    Complete,
}

#[cfg(test)]
mod tests {
    use serde::de::DeserializeSeed as _;
    use serde_json::json;

    use super::SourcedValueSeed;
    use crate::{OriginReaderLimits, SourceValueState};

    fn seed() -> SourcedValueSeed<String> {
        SourcedValueSeed::new(OriginReaderLimits {
            fields: 1,
            metadata_bytes: 1024,
            path_bytes: 64,
            field_uses: 2,
            reference_bytes: 12,
        })
    }

    #[test]
    fn populated_and_empty_texts_keep_typed_state_and_origin() -> Result<(), serde_json::Error> {
        for text in ["", "exact"] {
            let value = seed().deserialize(json!({
                "state": "present",
                "value": text,
                "containsEncryptedSource": true,
                "sourceUsesRefs": ["u1"],
                "sourceSetComplete": true,
            }))?;
            assert_eq!(
                value.state(),
                SourceValueState::Present,
                "authored string state"
            );
            assert_eq!(
                value.value().map(String::as_str),
                Some(text),
                "exact authored value"
            );
            assert_eq!(
                value.origin().contains_encrypted_source(),
                Some(true),
                "validated marker"
            );
        }
        Ok(())
    }

    #[test]
    fn contradictory_or_incomplete_wire_values_are_refused() {
        for (state, value, encrypted, references, complete) in [
            ("present", json!(null), json!(null), json!([]), false),
            ("unreadable", json!("text"), json!(null), json!([]), false),
            ("absent", json!(null), json!(true), json!([]), false),
            ("absent", json!(null), json!(false), json!(["u1"]), false),
            (
                "absent",
                json!(null),
                json!(true),
                json!(["u1", "u1"]),
                true,
            ),
            (
                "absent",
                json!(null),
                json!(true),
                json!(["u1", "u2", "u3"]),
                true,
            ),
        ] {
            let result = seed().deserialize(json!({
                "state": state,
                "value": value,
                "containsEncryptedSource": encrypted,
                "sourceUsesRefs": references,
                "sourceSetComplete": complete,
            }));
            assert!(result.is_err(), "reject malformed state or origin carrier");
        }
    }
}
