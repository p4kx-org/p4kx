// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MusicFloat {
    raw_bits: String,
    finite: Option<f64>,
}

impl MusicFloat {
    pub(crate) fn from_f32(bits: u32) -> Self {
        let number = f32::from_bits(bits);
        Self {
            raw_bits: format!("{bits:08x}"),
            finite: number.is_finite().then_some(f64::from(number)),
        }
    }

    pub(crate) fn from_f64(bits: u64) -> Self {
        let number = f64::from_bits(bits);
        Self {
            raw_bits: format!("{bits:016x}"),
            finite: number.is_finite().then_some(number),
        }
    }
}
