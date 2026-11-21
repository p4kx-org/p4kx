// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::de::{DeserializeSeed, Deserializer, Visitor};

use crate::FieldPath;
use crate::origin_reader_limits::ReadBudget;

/// Reads an escaped field path under an explicit consumer byte policy.
#[derive(Debug, Clone, Copy)]
pub struct FieldPathSeed {
    max_bytes: usize,
}

impl FieldPathSeed {
    /// Sets the maximum copied UTF-8 path length for this read.
    #[must_use]
    pub const fn new(max_bytes: usize) -> Self {
        Self { max_bytes }
    }
}

impl<'de> DeserializeSeed<'de> for FieldPathSeed {
    type Value = FieldPath;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        let budget = ReadBudget::new(self.max_bytes.saturating_add(size_of::<FieldPath>()));
        PathSeed {
            allowed: true,
            max_bytes: self.max_bytes,
            budget: &budget,
        }
        .deserialize(deserializer)
    }
}

pub(crate) struct PathSeed<'a> {
    pub(crate) allowed: bool,
    pub(crate) max_bytes: usize,
    pub(crate) budget: &'a ReadBudget,
}

impl<'de> DeserializeSeed<'de> for PathSeed<'_> {
    type Value = FieldPath;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_str(self)
    }
}

impl Visitor<'_> for PathSeed<'_> {
    type Value = FieldPath;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an escaped JSON Pointer within the explicit reader policy")
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<FieldPath, E> {
        if !self.allowed {
            return Err(E::custom("field count exceeds the reader policy"));
        }
        if value.len() > self.max_bytes {
            return Err(E::custom("field path exceeds the reader byte policy"));
        }
        self.budget
            .charge(value.len().saturating_add(size_of::<FieldPath>()))?;
        FieldPath::parse(value).map_err(E::custom)
    }
}
