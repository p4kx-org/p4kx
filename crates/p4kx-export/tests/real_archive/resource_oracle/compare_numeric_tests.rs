// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::json;

use super::{Raw, Result, scalar};

#[test]
fn raw_quality_integer_bounds_require_the_original_tag_and_exact_payload() -> Result<()> {
    for field in ["min", "max", "start", "end", "mappedValue"] {
        let path = format!("properties[1].quality.{field}");
        for expected in [i32::MIN, 0, 16_777_217, i32::MAX] {
            let raw = Raw::Int32(expected);
            assert!(
                scalar(&raw, &json!({"type":"int32", "value":expected}), &path)?,
                "the original Int32 width and exact bound are checked at {field}"
            );
            for invalid in [
                json!(expected),
                json!({"type":"int16", "value":expected}),
                json!({"type":"uint32", "value":expected}),
                json!({"type":"int32", "value":i64::from(expected) + 1}),
            ] {
                assert!(
                    !scalar(&raw, &invalid, &path)?,
                    "a bare number, wrong storage width/sign or different value cannot replace the original {field}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn raw_quality_float_bounds_require_exact_width_bits_and_finite_projection() -> Result<()> {
    let raw = Raw::Double(1.25);
    for field in [
        "min",
        "max",
        "start",
        "end",
        "mappedValue",
        "mean",
        "stddev",
    ] {
        let path = format!("properties[1].quality.{field}");
        assert!(
            scalar(
                &raw,
                &json!({"type":"f64", "rawBits":"3ff4000000000000", "finite":1.25}),
                &path
            )?,
            "diagnostic raw bounds can retain original Double storage at {field}"
        );
        for invalid in [
            json!({"rawBits":"3ff4000000000000", "finite":1.25}),
            json!({"type":"f32", "rawBits":"3ff4000000000000", "finite":1.25}),
            json!({"type":"f64", "rawBits":"3ff4000000000001", "finite":1.25}),
            json!({"type":"f64", "rawBits":"3ff4000000000000", "finite":1.5}),
        ] {
            assert!(
                !scalar(&raw, &invalid, &path)?,
                "the exact original Double tag, IEEE bits and finite projection remain independent checks at {field}"
            );
        }
    }
    Ok(())
}
