// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::MediaRecord;
use serde_json::Value;

use super::material_support::TestResult;

pub(super) fn field<'a>(
    value: &'a Value,
    pointer: &str,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("missing field {pointer}").into())
}

pub(super) fn reference(
    record: &MediaRecord,
    ordinal: usize,
) -> Result<&Value, Box<dyn std::error::Error>> {
    field(&record.data, "/payload/references")?
        .as_array()
        .ok_or("reference array")?
        .get(ordinal)
        .ok_or("reference ordinal")?
        .get("reference")
        .ok_or_else(|| "reference object".into())
}

pub(super) fn status(reference: &Value, expected: &str) -> TestResult {
    assert_eq!(
        field(reference, "/resolution/status")?.as_str(),
        Some(expected),
        "source-bound reference resolution category"
    );
    Ok(())
}

pub(super) fn target_index(reference: &Value) -> Result<u64, Box<dyn std::error::Error>> {
    field(reference, "/resolution/target/source/entry/entry_index")?
        .as_u64()
        .ok_or_else(|| "physical target index".into())
}
