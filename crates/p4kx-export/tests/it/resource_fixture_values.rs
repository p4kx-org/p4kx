// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestInstance, TestValue};

/// Builds one independently owned synthetic struct value.
pub(crate) fn owned(
    type_name: &str,
    fields: impl IntoIterator<Item = (&'static str, TestValue)>,
) -> TestValue {
    let mut value = TestInstance::new(type_name);
    for (name, field) in fields {
        value = value.value(name, field);
    }
    TestValue::Struct(value)
}

/// Builds a Normal source with integer bounds and floating-point parameters.
pub(crate) fn normal(min: i32, max: i32, mean: f32, stddev: f32) -> TestValue {
    owned(
        "CraftingQualityDistributionNormal",
        [
            ("min", TestValue::Int32(min)),
            ("max", TestValue::Int32(max)),
            ("mean", TestValue::Single(mean)),
            ("stddev", TestValue::Single(stddev)),
        ],
    )
}

/// Builds a Uniform source with exact integer bounds.
pub(crate) fn uniform(min: i32, max: i32) -> TestValue {
    owned(
        "CraftingQualityDistributionUniform",
        [
            ("min", TestValue::Int32(min)),
            ("max", TestValue::Int32(max)),
        ],
    )
}

/// Builds a quality source that names a definition Record.
pub(crate) fn quality_reference(type_name: &str, field: &'static str, guid: u128) -> TestValue {
    owned(type_name, [(field, TestValue::Reference(guid))])
}

/// Builds a single exact-integer quantization band.
pub(crate) fn quantization() -> TestValue {
    owned(
        "CraftingQualityQuantization",
        [(
            "bands",
            TestValue::Array(vec![owned(
                "CraftingQualityQuantizationBand",
                [
                    ("start", TestValue::Int32(0)),
                    ("end", TestValue::Int32(42)),
                    ("mappedValue", TestValue::Int32(11)),
                ],
            )]),
        )],
    )
}

/// Builds an override entry with an independently authored distribution source.
pub(crate) fn override_entry(location: u128, distribution: TestValue) -> TestValue {
    owned(
        "CraftingQualityLocationOverrideEntry",
        [
            ("location", TestValue::Reference(location)),
            ("qualityDistribution", distribution),
        ],
    )
}
