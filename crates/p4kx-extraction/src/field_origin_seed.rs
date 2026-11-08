// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::Deserialize;
use serde::de::{DeserializeSeed, Deserializer, MapAccess, Visitor};

use crate::origin_reader_limits::ReadBudget;
use crate::source_use_wire::ReferencesSeed;
use crate::{FieldOrigin, OriginReaderLimits};

/// Reads immediate origin metadata under an explicit finite consumer policy.
#[derive(Debug, Clone, Copy)]
pub struct FieldOriginSeed {
    limits: OriginReaderLimits,
}

impl FieldOriginSeed {
    /// Sets the limits for this read without limiting any producer.
    #[must_use]
    pub const fn new(limits: OriginReaderLimits) -> Self {
        Self { limits }
    }
}

impl<'de> DeserializeSeed<'de> for FieldOriginSeed {
    type Value = FieldOrigin;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        let budget = ReadBudget::new(self.limits.metadata_bytes);
        OriginSeed {
            limits: self.limits,
            budget: &budget,
        }
        .deserialize(deserializer)
    }
}

pub(crate) struct OriginSeed<'a> {
    pub(crate) limits: OriginReaderLimits,
    pub(crate) budget: &'a ReadBudget,
}

impl<'de> DeserializeSeed<'de> for OriginSeed<'_> {
    type Value = FieldOrigin;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        self.budget.charge(size_of::<FieldOrigin>())?;
        deserializer.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for OriginSeed<'_> {
    type Value = FieldOrigin;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an immediate origin with all three correlated marker fields")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<FieldOrigin, M::Error> {
        use serde::de::Error as _;
        let mut encrypted = None;
        let mut references = None;
        let mut complete = None;
        while let Some(field) = map.next_key::<OriginField>()? {
            match field {
                OriginField::Encrypted => {
                    if encrypted.is_some() {
                        return Err(M::Error::duplicate_field("containsEncryptedSource"));
                    }
                    encrypted = Some(map.next_value::<Option<bool>>()?);
                }
                OriginField::References => {
                    if references.is_some() {
                        return Err(M::Error::duplicate_field("sourceUsesRefs"));
                    }
                    references = Some(map.next_value_seed(ReferencesSeed {
                        limits: self.limits,
                        budget: self.budget,
                    })?);
                }
                OriginField::Complete => {
                    if complete.is_some() {
                        return Err(M::Error::duplicate_field("sourceSetComplete"));
                    }
                    complete = Some(map.next_value::<bool>()?);
                }
                OriginField::Other => {
                    map.next_value::<serde::de::IgnoredAny>()?;
                }
            }
        }
        FieldOrigin::from_wire(
            encrypted.ok_or_else(|| M::Error::missing_field("containsEncryptedSource"))?,
            references.ok_or_else(|| M::Error::missing_field("sourceUsesRefs"))?,
            complete.ok_or_else(|| M::Error::missing_field("sourceSetComplete"))?,
        )
        .map_err(M::Error::custom)
    }
}

enum OriginField {
    Encrypted,
    References,
    Complete,
    Other,
}

impl<'de> Deserialize<'de> for OriginField {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Field;
        impl Visitor<'_> for Field {
            type Value = OriginField;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an origin metadata field name")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<OriginField, E> {
                Ok(match value {
                    "containsEncryptedSource" => OriginField::Encrypted,
                    "sourceUsesRefs" => OriginField::References,
                    "sourceSetComplete" => OriginField::Complete,
                    _ => OriginField::Other,
                })
            }
        }
        deserializer.deserialize_identifier(Field)
    }
}
