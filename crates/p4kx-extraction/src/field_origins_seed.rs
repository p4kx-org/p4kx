// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::de::{DeserializeSeed, Deserializer, MapAccess, Visitor};

use crate::field_origin_seed::OriginSeed;
use crate::field_path_seed::PathSeed;
use crate::origin_reader_limits::ReadBudget;
use crate::{FieldOrigins, OriginReaderLimits};

/// Reads a field-addressed companion under an explicit finite consumer policy.
#[derive(Debug, Clone, Copy)]
pub struct FieldOriginsSeed {
    limits: OriginReaderLimits,
}

impl FieldOriginsSeed {
    /// Sets field, path, reference and total metadata limits for this read.
    #[must_use]
    pub const fn new(limits: OriginReaderLimits) -> Self {
        Self { limits }
    }
}

impl<'de> DeserializeSeed<'de> for FieldOriginsSeed {
    type Value = FieldOrigins;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        let budget = ReadBudget::new(self.limits.metadata_bytes);
        deserializer.deserialize_map(Origins {
            limits: self.limits,
            budget: &budget,
        })
    }
}

struct Origins<'a> {
    limits: OriginReaderLimits,
    budget: &'a ReadBudget,
}

impl<'de> Visitor<'de> for Origins<'_> {
    type Value = FieldOrigins;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a field-origin map within the explicit reader policy")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<FieldOrigins, M::Error> {
        use serde::de::Error as _;
        if map
            .size_hint()
            .is_some_and(|count| count > self.limits.fields)
        {
            return Err(M::Error::custom("field count exceeds the reader policy"));
        }
        let mut result = FieldOrigins::new();
        let mut count = 0;
        while let Some(path) = map.next_key_seed(PathSeed {
            allowed: count < self.limits.fields,
            max_bytes: self.limits.path_bytes,
            budget: self.budget,
        })? {
            let origin = map.next_value_seed(OriginSeed {
                limits: self.limits,
                budget: self.budget,
            })?;
            result.insert(path, origin).map_err(M::Error::custom)?;
            count += 1;
        }
        Ok(result)
    }
}
