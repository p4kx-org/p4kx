// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use p4kx_extraction::DecimalU64;
use serde::de::{Deserializer, Visitor};

/// Reads the canonical decimal wire spelling without a JSON-number conversion.
pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<DecimalU64, D::Error> {
    deserializer.deserialize_str(Decimal).map(DecimalU64::new)
}

/// Checks a descriptive decimal coordinate without creating a source binding.
pub(crate) fn parse(value: &str) -> Result<u64, &'static str> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("invalid canonical decimal string");
    }
    value
        .parse()
        .map_err(|_| "decimal coordinate exceeds the unsigned 64-bit range")
}

struct Decimal;

impl Visitor<'_> for Decimal {
    type Value = u64;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a canonical unsigned 64-bit decimal string")
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<u64, E> {
        parse(value).map_err(E::custom)
    }
}
