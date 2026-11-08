// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::fmt;

use serde::de::{DeserializeSeed, Deserializer, SeqAccess, Visitor};

use crate::origin_reader_limits::ReadBudget;
use crate::{OriginReaderLimits, SourceUseRef};

pub(crate) struct ReferencesSeed<'a> {
    pub(crate) limits: OriginReaderLimits,
    pub(crate) budget: &'a ReadBudget,
}

impl<'de> DeserializeSeed<'de> for ReferencesSeed<'_> {
    type Value = Vec<SourceUseRef>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for ReferencesSeed<'_> {
    type Value = Vec<SourceUseRef>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("distinct source-use references within the explicit reader policy")
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut sequence: S) -> Result<Self::Value, S::Error> {
        use serde::de::Error as _;
        if sequence
            .size_hint()
            .is_some_and(|count| count > self.limits.field_uses)
        {
            return Err(S::Error::custom(
                "source-use count exceeds the reader policy",
            ));
        }
        let mut references = BTreeSet::new();
        while let Some(reference) = sequence.next_element_seed(ReferenceSeed {
            allowed: references.len() < self.limits.field_uses,
            max_bytes: self.limits.reference_bytes,
            budget: self.budget,
        })? {
            if !references.insert(reference) {
                return Err(S::Error::custom(
                    "field source-use references are duplicated",
                ));
            }
        }
        Ok(references.into_iter().collect())
    }
}

struct ReferenceSeed<'a> {
    allowed: bool,
    max_bytes: usize,
    budget: &'a ReadBudget,
}

impl<'de> DeserializeSeed<'de> for ReferenceSeed<'_> {
    type Value = SourceUseRef;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_str(self)
    }
}

impl Visitor<'_> for ReferenceSeed<'_> {
    type Value = SourceUseRef;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a nonempty document-local source-use reference")
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<SourceUseRef, E> {
        if !self.allowed {
            return Err(E::custom("source-use count exceeds the reader policy"));
        }
        if value.is_empty() || value.len() > self.max_bytes {
            return Err(E::custom(
                "source-use reference exceeds the reader byte policy",
            ));
        }
        self.budget
            .charge(value.len().saturating_add(size_of::<SourceUseRef>()))?;
        let mut owned = String::new();
        owned.try_reserve_exact(value.len()).map_err(E::custom)?;
        owned.push_str(value);
        Ok(SourceUseRef(owned))
    }
}
