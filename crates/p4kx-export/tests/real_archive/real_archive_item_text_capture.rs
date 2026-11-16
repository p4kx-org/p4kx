// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::Deserialize;
use serde::de::value::{MapAccessDeserializer, SeqAccessDeserializer};
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};

#[derive(Clone, Copy)]
pub(super) enum Projection {
    Whole,
    Fields(&'static [(&'static str, Self)]),
    Array(&'static Self),
    Carrier {
        value: &'static Self,
        additional: &'static [(&'static str, Self)],
    },
}

use Projection::{Array, Carrier, Fields, Whole};

const LOCALIZATION: Projection = Fields(&[("name", Whole), ("description", Whole)]);

const COMPONENT: Projection = Fields(&[
    ("kind", Whole),
    ("componentOrdinal", Whole),
    ("sourceType", Whole),
    (
        "value",
        Carrier {
            value: &Fields(&[("localization", Projection::carrier(&LOCALIZATION))]),
            additional: &[("displayName", Whole), ("displayType", Whole)],
        },
    ),
]);

pub(super) const ITEM_DEFINITIONS: Projection = Fields(&[
    ("availability", Whole),
    (
        "rows",
        Array(&Fields(&[
            ("recordIndex", Whole),
            ("guid", Whole),
            ("recordPath", Whole),
            ("sourceType", Whole),
            ("components", Projection::carrier(&Array(&COMPONENT))),
        ])),
    ),
]);

impl Projection {
    const fn carrier(value: &'static Self) -> Self {
        Self::Carrier {
            value,
            additional: &[],
        }
    }

    fn field(self, key: &str) -> Option<Self> {
        let fields = match self {
            Self::Fields(fields) => fields,
            Self::Carrier { value, additional } => match key {
                "state" | "containsEncryptedSource" | "sourceSetComplete" | "sourceUsesRefs" => {
                    return Some(Self::Whole);
                }
                "value" => return Some(*value),
                _ => additional,
            },
            Self::Whole | Self::Array(_) => return None,
        };
        fields
            .iter()
            .find_map(|&(name, projection)| (name == key).then_some(projection))
    }
}

impl<'de> DeserializeSeed<'de> for Projection {
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        if matches!(self, Self::Whole) {
            Value::deserialize(deserializer)
        } else {
            deserializer.deserialize_any(self)
        }
    }
}

impl<'de> Visitor<'de> for Projection {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a catalog Item text projection")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        if matches!(self, Self::Array(_)) {
            return Value::deserialize(MapAccessDeserializer::new(map));
        }
        let mut kept = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if let Some(projection) = self.field(&key) {
                kept.insert(key, map.next_value_seed(projection)?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(Value::Object(kept))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let Self::Array(item) = self else {
            return Value::deserialize(SeqAccessDeserializer::new(sequence));
        };
        let mut kept = Vec::new();
        while let Some(item) = sequence.next_element_seed(*item)? {
            kept.push(item);
        }
        Ok(Value::Array(kept))
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::from(value))
    }
}
