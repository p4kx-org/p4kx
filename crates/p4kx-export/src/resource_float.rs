// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Serialize, Serializer};

use crate::schema_fields;

/// An exact source Single with a nullable finite decimal projection.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceFloat32 {
    #[serde(serialize_with = "bits32")]
    #[schemars(
        with = "String",
        regex(pattern = "^[0-9a-f]{8}$"),
        length(min = 8, max = 8)
    )]
    pub(crate) raw_bits: u32,
    pub(crate) finite: Option<f32>,
}

impl ResourceFloat32 {
    pub(crate) fn new(value: f32) -> Self {
        let bits = value.to_bits();
        Self {
            raw_bits: bits,
            finite: value.is_finite().then_some(value),
        }
    }
}

/// An exact source Double with a nullable finite decimal projection.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceFloat64 {
    #[serde(serialize_with = "bits64")]
    #[schemars(
        with = "String",
        regex(pattern = "^[0-9a-f]{16}$"),
        length(min = 16, max = 16)
    )]
    pub(crate) raw_bits: u64,
    pub(crate) finite: Option<f64>,
}

impl ResourceFloat64 {
    pub(crate) fn new(value: f64) -> Self {
        let bits = value.to_bits();
        Self {
            raw_bits: bits,
            finite: value.is_finite().then_some(value),
        }
    }
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "Serde serialize_with callbacks borrow their field values."
)]
fn bits32<S: Serializer>(value: &u32, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&format_args!("{value:08x}"))
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "Serde serialize_with callbacks borrow their field values."
)]
fn bits64<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&format_args!("{value:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ieee_payload_bits_and_negative_zero_survive_the_decimal_projection() {
        for bits in [
            0x8000_0000,
            0x7f80_0000,
            0xff80_0000,
            0x7fc0_0001,
            0x7f80_0001,
        ] {
            let value = ResourceFloat32::new(f32::from_bits(bits));
            let json = serde_json::to_value(value).expect("serialize typed Single fixture");
            assert_eq!(json["rawBits"], format!("{bits:08x}"));
            if bits == 0x8000_0000 {
                assert_eq!(json["finite"].to_string(), "-0.0");
            } else {
                assert!(
                    json["finite"].is_null(),
                    "nonfinite decimal projection stays null"
                );
            }
        }
        let bits = 0x7ff0_0000_0000_0001;
        let json = serde_json::to_value(ResourceFloat64::new(f64::from_bits(bits)))
            .expect("serialize typed Double fixture");
        assert_eq!(json["rawBits"], "7ff0000000000001");
        assert!(
            json["finite"].is_null(),
            "nonfinite Double remains present through its raw bits"
        );
    }
}
