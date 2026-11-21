// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use jsonschema::Validator;
use serde_json::{Value, json};

use crate::catalog_schema_support;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const RAW_NUMBER: &str = "/definitions/qualityDistributions/rows/0/qualityDistribution/value/payload/value/value/value/min/value";
const ELIGIBILITY: &str = "/projections/qualityQuantizationResources/rows/0/eligibility/value";

struct IntegerCase {
    kind: &'static str,
    minimum: Value,
    maximum: Value,
    below: Value,
    above: Value,
}

fn integer_cases() -> [IntegerCase; 2] {
    [
        IntegerCase {
            kind: "int32",
            minimum: json!(i32::MIN),
            maximum: json!(i32::MAX),
            below: json!(i64::from(i32::MIN) - 1),
            above: json!(i64::from(i32::MAX) + 1),
        },
        IntegerCase {
            kind: "uint32",
            minimum: json!(u32::MIN),
            maximum: json!(u32::MAX),
            below: json!(i64::from(u32::MIN) - 1),
            above: json!(u64::from(u32::MAX) + 1),
        },
    ]
}

#[test]
fn raw_32_bit_integer_tags_have_explicit_exact_schema_bounds() -> TestResult {
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    let catalog = catalog_schema_support::minimal_catalog();
    for case in integer_cases() {
        assert_explicit_bounds(&schema, &case)?;
        for value in [&case.minimum, &case.maximum] {
            assert!(
                validator.is_valid(&with_token(&catalog, case.kind, value)?),
                "the complete {} storage range includes {value}",
                case.kind
            );
        }
        for value in [&case.below, &case.above, &json!(1.5)] {
            assert!(
                !validator.is_valid(&with_token(&catalog, case.kind, value)?),
                "the {} tag rejects fractional or out-of-range payload {value}",
                case.kind
            );
        }
    }
    Ok(())
}

fn with_token(catalog: &Value, kind: &str, value: &Value) -> TestResult<Value> {
    let mut changed = catalog.clone();
    *changed
        .pointer_mut(RAW_NUMBER)
        .ok_or("raw numeric fixture")? = json!({"type":kind, "value":value});
    Ok(changed)
}

fn assert_explicit_bounds(schema: &Value, case: &IntegerCase) -> TestResult {
    let variants = schema
        .pointer("/$defs/ResourceNumber/oneOf")
        .and_then(Value::as_array)
        .ok_or("raw number schema variants")?;
    let variant = variants
        .iter()
        .find(|variant| {
            variant
                .pointer("/properties/type/const")
                .and_then(Value::as_str)
                == Some(case.kind)
        })
        .ok_or("tagged raw integer schema")?;
    assert_eq!(
        variant.pointer("/properties/value/minimum"),
        Some(&case.minimum),
        "{} has an explicit typed lower bound",
        case.kind
    );
    assert_eq!(
        variant.pointer("/properties/value/maximum"),
        Some(&case.maximum),
        "{} has an explicit typed upper bound",
        case.kind
    );
    Ok(())
}

#[test]
fn quantization_eligibility_keeps_closed_status_and_reason_in_a_compound_union() -> TestResult {
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    let catalog = catalog_schema_support::minimal_catalog();
    assert!(
        schema
            .pointer("/$defs/QuantizationEligibility/x-p4kx-vocabularyClass")
            .is_none(),
        "a tagged object union is not a finite scalar vocabulary"
    );
    assert_eq!(
        schema.pointer("/$defs/QuantizationIneligibility/x-p4kx-vocabularyClass"),
        Some(&json!("control")),
        "the finite reason strings keep their control class"
    );
    assert!(
        validator.is_valid(&with_eligibility(&catalog, json!({"status":"eligible"}))?),
        "the eligible unit variant remains valid"
    );
    for reason in ["noInlineBands", "emptyBands", "invalidBands", "unavailable"] {
        assert!(
            validator.is_valid(&with_eligibility(
                &catalog,
                json!({"status":"notEligible", "reason":reason})
            )?),
            "the exact known ineligibility reason {reason} remains valid"
        );
    }
    for invalid in [
        json!({"status":"futureStatus"}),
        json!({"status":"notEligible", "reason":"futureReason"}),
        json!({"status":"notEligible"}),
        json!({"status":"notEligible", "reason":false}),
        json!("eligible"),
    ] {
        assert!(
            !validator.is_valid(&with_eligibility(&catalog, invalid.clone())?),
            "the closed compound eligibility rejects {invalid}"
        );
    }
    Ok(())
}

fn with_eligibility(catalog: &Value, eligibility: Value) -> TestResult<Value> {
    let mut changed = catalog.clone();
    *changed
        .pointer_mut(ELIGIBILITY)
        .ok_or("inverse eligibility fixture")? = eligibility;
    Ok(changed)
}

pub(super) fn assert_unknown_quality_fields(validator: &Validator, catalog: &Value) -> TestResult {
    for family in ["qualityDistributions", "qualityQuantizations"] {
        let mut unknown = catalog.clone();
        unknown
            .pointer_mut(&format!("/definitions/{family}/rows/0"))
            .and_then(Value::as_object_mut)
            .ok_or("quality definition")?
            .insert("extra".to_owned(), json!(true));
        assert!(
            validator.is_valid(&unknown),
            "unknown optional definition fields in {family}"
        );
    }
    Ok(())
}
