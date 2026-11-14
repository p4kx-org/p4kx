// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::{TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::quality_v3_fixture_schema as schema;
use crate::quality_v3_fixture_values as values;
use crate::quality_v3_support as support;

#[test]
fn quality_v3_exports_f64_beyond_f32_without_an_obsolete_narrowing_failure() -> TestResult {
    let database = schema::schema().record(values::distribution(
        values::DISTRIBUTION,
        "wide",
        values::normal(
            TestValue::Int32(i32::MIN),
            TestValue::Double(1.0e300),
            TestValue::Double(1.25),
        ),
    ));
    let (built, catalog) = support::normal(&database)?;
    let row = support::row(&catalog, support::DISTRIBUTIONS, 0)?;
    let mean = support::at(row, &format!("{}/mean", support::CURVE))?;
    support::state(mean, "present")?;
    assert_eq!(support::at(mean, "/value/type")?, &json!("f64"));
    assert_eq!(
        support::at(mean, "/value/rawBits")?,
        &json!("7e37e43c8800759c")
    );
    assert_eq!(support::at(mean, "/value/finite")?, &json!(1.0e300));
    assert_eq!(
        support::at(&catalog, "/exportHealth/status")?,
        &json!("complete")
    );
    assert!(
        built.summary().is_complete(),
        "fully exported source precision is not an incomplete legacy f32 view"
    );
    assert_eq!(
        built
            .summary()
            .kind(Kind::QualityDistributions)
            .map(|kind| kind.count),
        Some(1)
    );
    Ok(())
}

#[test]
fn quality_v3_keeps_actual_ieee_width_sign_and_nan_payload_bits_in_normal_exports() -> TestResult {
    for (bits, expected, nonfinite) in [
        (0x8000_0000, "80000000", false),
        (0x7f80_0000, "7f800000", true),
        (0xff80_0000, "ff800000", true),
        (0xffc0_1234, "ffc01234", true),
        (0x7f80_0042, "7f800042", true),
    ] {
        assert_single(bits, expected, nonfinite)?;
    }
    for (bits, expected, nonfinite) in [
        (0x8000_0000_0000_0000, "8000000000000000", false),
        (0x7ff0_0000_0000_0000, "7ff0000000000000", true),
        (0xfff0_0000_0000_0000, "fff0000000000000", true),
        (0xfff8_0000_0000_1234, "fff8000000001234", true),
        (0x7ff0_0000_0000_0042, "7ff0000000000042", true),
    ] {
        assert_double(bits, expected, nonfinite)?;
    }
    Ok(())
}

fn assert_single(bits: u32, expected: &str, nonfinite: bool) -> TestResult {
    let database = schema::numeric_schema(TestType::Int32, TestType::Single, TestType::Single)
        .record(values::distribution(
            values::DISTRIBUTION,
            "single",
            values::normal(
                TestValue::Int32(0),
                TestValue::Single(f32::from_bits(bits)),
                TestValue::Single(1.25),
            ),
        ));
    assert_ieee(&support::normal(&database)?.1, "f32", expected, nonfinite)
}

fn assert_double(bits: u64, expected: &str, nonfinite: bool) -> TestResult {
    let database = schema::schema().record(values::distribution(
        values::DISTRIBUTION,
        "double",
        values::normal(
            TestValue::Int32(0),
            TestValue::Double(f64::from_bits(bits)),
            TestValue::Double(1.25),
        ),
    ));
    assert_ieee(&support::normal(&database)?.1, "f64", expected, nonfinite)
}
fn assert_ieee(catalog: &Value, width: &str, bits: &str, nonfinite: bool) -> TestResult {
    let row = support::row(catalog, support::DISTRIBUTIONS, 0)?;
    let mean = support::at(row, &format!("{}/mean", support::CURVE))?;
    support::state(mean, "present")?;
    support::marker(mean, Some(false))?;
    assert_eq!(
        support::at(mean, "/value/type")?,
        &json!(width),
        "the retained IEEE width matches the actual original source token"
    );
    assert_eq!(
        support::at(mean, "/value/rawBits")?,
        &json!(bits),
        "original source bits precede finite projection"
    );
    let finite = support::at(mean, "/value/finite")?;
    if nonfinite {
        assert_eq!(
            finite,
            &Value::Null,
            "nonfinite source bits have no finite decimal projection"
        );
    } else {
        assert_eq!(
            serde_json::to_string(finite)?,
            "-0.0",
            "negative zero remains a signed decimal projection"
        );
    }
    assert_eq!(
        support::at(catalog, "/exportHealth/status")?,
        &json!("complete"),
        "exact IEEE token preservation is a complete supported export"
    );
    Ok(())
}

#[test]
fn quality_v3_retains_wrong_width_numeric_tokens_with_separate_declared_field_failure() -> TestResult
{
    let curve = || {
        values::normal(
            TestValue::UInt64(u64::MAX),
            TestValue::Double(7.5),
            TestValue::Double(1.25),
        )
    };
    let database = schema::numeric_schema(TestType::UInt64, TestType::Double, TestType::Double)
        .record(values::distribution(
            values::DISTRIBUTION,
            "wrong_width",
            curve(),
        ))
        .record(values::resource(
            values::RESOURCE_A,
            "raw_curve",
            vec![values::distribution_property(curve())],
        ));
    support::assert_normal_refusal(&database)?;
    let (_, catalog) = support::diagnostic(&database)?;
    let row = support::row(&catalog, support::DISTRIBUTIONS, 0)?;
    let minimum = support::at(row, &format!("{}/min", support::CURVE))?;
    support::state(minimum, "present")?;
    assert_eq!(
        support::at(minimum, "/value")?,
        &json!({"type": "uint64", "value": "18446744073709551615"})
    );
    support::marker(minimum, Some(false))?;
    let resource = support::row(&catalog, "definitions/resourceTypes", 1)?;
    let resource_minimum = support::at(
        resource,
        "/properties/value/0/value/payload/value/qualityDistribution/value/payload/value/value/value/min",
    )?;
    support::state(resource_minimum, "present")?;
    assert_eq!(
        support::at(resource_minimum, "/value")?,
        support::at(minimum, "/value")?,
        "both source owners use the same existing exact numeric contract"
    );
    support::assert_field_error(
        &catalog,
        "qualityDistribution.min",
        "CraftingQualityDistributionNormal",
    )?;
    assert_wrong_width_health(&catalog)?;
    Ok(())
}

fn assert_wrong_width_health(catalog: &Value) -> TestResult {
    let health = support::kind_health(catalog, "qualityDistributions")?;
    for (field, expected) in [
        ("readSuccessful", 1),
        ("projected", 1),
        ("omittedAfterProjectionFailure", 0),
    ] {
        assert_eq!(
            support::at(health, &format!("/sourceRecords/{field}"))?,
            &json!(expected),
            "retained raw owner and invalid declared field have distinct accounting"
        );
    }
    assert_eq!(
        support::at(catalog, "/exportHealth/status")?,
        &json!("incomplete"),
        "the declared-width mismatch remains incomplete despite exact raw preservation"
    );
    Ok(())
}
