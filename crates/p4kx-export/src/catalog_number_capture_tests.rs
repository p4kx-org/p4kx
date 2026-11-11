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

#[test]
fn equal_named_loot_values_keep_distinct_original_occurrence_receipts() {
    let capture = NumberCapture::default();
    let value = Value::Single(f32::from_bits(0x7fc0_0017));
    for ordinal in [3, 4, 3] {
        capture.retain_at(
            7,
            super::NumberOccurrence::LootValue { ordinal },
            || "filter.same".to_owned(),
            "PoolFilter_Custom",
            ResourceNumberContract::Number,
            &value,
        );
    }
    let receipts = capture.finish();
    assert_eq!(
        receipts.len(),
        2,
        "equal names/tokens at different original positions remain independent"
    );
    for (receipt, ordinal) in receipts.iter().zip([3, 4]) {
        assert_eq!(
            receipt.occurrence,
            super::NumberOccurrence::LootValue { ordinal },
            "actual source occurrence, not a named-path alias"
        );
        assert_eq!(
            receipt.token,
            FloatToken::Single(0x7fc0_0017),
            "source width and payload bits remain exact"
        );
    }
}

#[test]
fn finite_loot_values_do_not_allocate_conditional_receipt_paths() {
    let capture = NumberCapture::default();
    let materialized = std::cell::Cell::new(false);
    capture.retain_at(
        7,
        super::NumberOccurrence::LootValue { ordinal: 3 },
        || {
            materialized.set(true);
            "filter.same".to_owned()
        },
        "PoolFilter_Custom",
        ResourceNumberContract::Number,
        &Value::Double(1.0),
    );
    assert!(
        !materialized.get(),
        "finite source facts do not require nonfinite-loss receipt paths"
    );
    assert!(
        capture.finish().is_empty(),
        "the complete scalar DTO carries finite data without a conditional receipt"
    );
}
