// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::de::value::{Error, MapDeserializer, SeqDeserializer};
use serde::de::{Deserializer, IntoDeserializer, Visitor};

/// Moves captured typed fields through the shared canonical origin seed.
pub(crate) fn deserializer<'de>(
    marker: Option<bool>,
    references: Vec<String>,
    complete: bool,
) -> impl Deserializer<'de, Error = Error> {
    MapDeserializer::new(
        [
            ("containsEncryptedSource", OriginValue::Marker(marker)),
            ("sourceUsesRefs", OriginValue::References(references)),
            ("sourceSetComplete", OriginValue::Complete(complete)),
        ]
        .into_iter(),
    )
}

enum OriginValue {
    Marker(Option<bool>),
    References(Vec<String>),
    Complete(bool),
}

impl IntoDeserializer<'_, Error> for OriginValue {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'de> Deserializer<'de> for OriginValue {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Self::Marker(Some(value)) => visitor.visit_some(value.into_deserializer()),
            Self::Marker(None) => visitor.visit_none(),
            Self::References(references) => {
                visitor.visit_seq(SeqDeserializer::new(references.into_iter()))
            }
            Self::Complete(value) => visitor.visit_bool(value),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map struct
        enum identifier ignored_any
    }
}
