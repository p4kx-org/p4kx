// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::{TestResult, option, replace, with_value};

#[test]
fn raw_option_text_ids_preserve_empty_placeholder_and_null_tokens() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for text_id in [Value::Null, json!(""), json!("@LOC_UNINITIALIZED")] {
        let mut value = option(&json!(1), &json!(0));
        replace(&mut value, "textId", text_id)?;
        assert!(
            validator.is_valid(&with_value(
                &json!({"type": "integer", "options": [value]})
            )?),
            "raw option textId availability does not depend on presentation filtering"
        );
    }
    Ok(())
}

#[test]
fn option_display_name_requires_the_direct_sourced_text_carrier() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for legacy in [Value::Null, json!({"key": null, "en": null, "de": null})] {
        let mut value = option(&json!(1), &json!(0));
        replace(&mut value, "displayName", legacy)?;
        assert!(
            !validator.is_valid(&with_value(
                &json!({"type": "integer", "options": [value]})
            )?),
            "an option's required direct text cell cannot be null or a bilingual object",
        );
    }
    Ok(())
}
