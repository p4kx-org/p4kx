// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;

use super::{FloatToken, NumberCapture};
use crate::resource_raw_policy::ResourceNumberContract;

#[test]
fn repeated_getters_do_not_multiply_one_retained_named_slot_receipt() {
    let capture = NumberCapture::default();
    let value = Value::Double(f64::from_bits(0x7ff8_0000_0000_1234));
    capture.retain(
        7,
        || "qualityDistribution.mean".to_owned(),
        "CraftingQualityDistributionNormal",
        ResourceNumberContract::Number,
        &value,
    );
    capture.retain(
        7,
        || "qualityDistribution.mean".to_owned(),
        "CraftingQualityDistributionNormal",
        ResourceNumberContract::Number,
        &value,
    );
    capture.retain(
        7,
        || "qualityDistribution.stddev".to_owned(),
        "CraftingQualityDistributionNormal",
        ResourceNumberContract::Number,
        &value,
    );
    let retained = capture.finish();
    assert_eq!(
        retained.len(),
        2,
        "different retained fields remain independent; repeated reads of one DTO slot are one receipt"
    );
    assert!(
        retained.iter().all(|number| number.record_index == 7
            && number.token == FloatToken::Double(0x7ff8_0000_0000_1234)),
        "receipts keep original physical identity and exact source bits"
    );
}

#[test]
fn finite_values_do_not_materialize_optional_nonfinite_receipt_coordinates() {
    let capture = NumberCapture::default();
    let materialized = std::cell::Cell::new(false);
    capture.retain(
        7,
        || {
            materialized.set(true);
            "qualityDistribution.mean".to_owned()
        },
        "CraftingQualityDistributionNormal",
        ResourceNumberContract::Number,
        &Value::Double(1.0),
    );
    assert!(
        !materialized.get(),
        "finite raw values need no conditional-loss receipt metadata"
    );
    assert!(
        capture.finish().is_empty(),
        "no nonfinite proof token was required"
    );
}
