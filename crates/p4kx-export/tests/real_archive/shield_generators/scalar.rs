// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Reference, Value as Original};
use serde_json::Value;

use crate::original_item_numbers as numbers;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
use crate::real_archive_item_components::cell;
use crate::real_archive_item_durability as proof;

pub(super) fn compare(raw: &Original<'_>, output: &Value) -> TestResult {
    let kind = proof::text(output, "/type")?;
    let value = cell(output, "value")?;
    let matches = match raw {
        Original::Bool(original) => kind == "boolean" && value.as_bool() == Some(*original),
        Original::Enum(original) => kind == "enum" && value.as_str() == Some(*original),
        Original::String(original) => kind == "string" && text_matches(value, original)?,
        Original::Locale(original) => kind == "locale" && text_matches(value, original)?,
        Original::Guid(original) => kind == "guid" && value.as_str() == Some(&original.to_string()),
        Original::Reference(original) => reference_matches(original, kind, value)?,
        _ => {
            kind == "number"
                && numbers::expected(raw)?.is_some_and(|number| numbers::equal(&number, value))
        }
    };
    assert!(
        matches,
        "the authored node retains the original scalar payload"
    );
    Ok(())
}

fn text_matches(value: &Value, original: &str) -> TestResult<bool> {
    Ok(
        matches!(proof::text(value, "/type")?, "public" | "gameText")
            && proof::text(value, "/sourceText")? == original,
    )
}

fn reference_matches(original: &Reference<'_>, kind: &str, value: &Value) -> TestResult<bool> {
    let identity = proof::number(value, "/targetRecordIndex/value")?
        == u64::from(original.record_index())
        && proof::text(value, "/recordPath/value")? == original.record_path();
    let target = match kind {
        "recordReference" => {
            original.location().is_none()
                && proof::text(value, "/status")? == "present"
                && proof::text(value, "/guid/value")? == original.guid().to_string()
                && proof::text(value, "/targetType/value")? == original.type_name()
        }
        "weakReference" => {
            let target_shape = match original.location() {
                Some(location) => {
                    proof::text(value, "/target/type")? == "instance"
                        && proof::text(value, "/target/location/value")? == location.to_string()
                }
                None => proof::text(value, "/target/type")? == "record",
            };
            target_shape
                && proof::text(value, "/recordGuid/value")? == original.guid().to_string()
                && proof::text(value, "/target/sourceType/value")? == original.type_name()
        }
        _ => false,
    };
    Ok(identity && target)
}
