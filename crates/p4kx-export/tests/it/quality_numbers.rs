// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Exact quality bounds and explicit floating-point projection failures.

use std::error::Error;

use p4kx_export::CatalogError;
use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use crate::catalog_crafting_support;

type TestResult = Result<(), Box<dyn Error>>;

#[derive(Clone, Copy)]
enum Numbers {
    Exact,
    SingleMinimum(f32),
    Overflow,
    NonFinite,
}

fn database(numbers: Numbers) -> DataForgeBuilder {
    let (minimum_type, minimum, mean, stddev) = match numbers {
        Numbers::Exact => (
            TestType::Int32,
            TestValue::Int32(i32::MIN),
            TestValue::Single(7.5),
            TestValue::Double(1.25),
        ),
        Numbers::SingleMinimum(value) => (
            TestType::Single,
            TestValue::Single(value),
            TestValue::Single(7.5),
            TestValue::Double(1.25),
        ),
        Numbers::Overflow => (
            TestType::Int32,
            TestValue::Int32(0),
            TestValue::Single(7.5),
            TestValue::Double(1.0e300),
        ),
        Numbers::NonFinite => (
            TestType::Int32,
            TestValue::Int32(0),
            TestValue::Single(f32::NAN),
            TestValue::Double(1.25),
        ),
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("CraftingQualityDistribution_Base"))
        .structure(
            TestStruct::new("CraftingQualityDistributionNormal")
                .parent("CraftingQualityDistribution_Base")
                .field("min", minimum_type)
                .field("max", TestType::Int32)
                .field("mean", TestType::Single)
                .field("stddev", TestType::Double),
        )
        .structure(TestStruct::new("CraftingQualityDistributionRecord").field(
            "qualityDistribution",
            TestType::StrongPointer("CraftingQualityDistribution_Base".to_owned()),
        ))
        .record(
            TestRecord::new(
                0x9101,
                "CraftingQualityDistributionRecord",
                "CraftingQualityDistributionRecord.FixtureNumbers",
                "libs/foundry/records/crafting/qualitydistribution/fixture_numbers.xml",
            )
            .value(
                "qualityDistribution",
                TestValue::Struct(
                    TestInstance::new("CraftingQualityDistributionNormal")
                        .value("min", minimum)
                        .value("max", TestValue::Int32(i32::MAX))
                        .value("mean", mean)
                        .value("stddev", stddev),
                ),
            ),
        )
}

#[test]
fn the_shared_public_curve_preserves_all_int32_bounds_and_finite_float_parameters() -> TestResult {
    let bytes = database(Numbers::Exact).build();
    let forge = DataForge::open(&bytes)?;
    let crafting = p4kx_export::read_crafting(&forge, None, None)?;
    let curve = crafting
        .quality_distributions
        .first()
        .and_then(|record| record.distribution.as_ref())
        .ok_or("the exact curve")?;
    assert_eq!((curve.min, curve.max), (Some(i32::MIN), Some(i32::MAX)));
    assert_eq!((curve.mean, curve.stddev), (Some(7.5), Some(1.25)));
    let (_, catalog) = catalog_crafting_support::build_from(&database(Numbers::Exact), None)?;
    assert_eq!(
        catalog["qualityDistributions"][0]["distribution"]["max"],
        i32::MAX
    );
    Ok(())
}

fn assert_projection_failure(numbers: Numbers, field: &str) -> TestResult {
    let database = database(numbers);
    let Err(error) = catalog_crafting_support::build_from(&database, None) else {
        return Err("unrepresentable typed data must refuse normal publication".into());
    };
    assert!(
        matches!(
            error.downcast_ref::<CatalogError>(),
            Some(CatalogError::IncompleteExport { .. })
        ),
        "invalid numeric data blocks normal publication"
    );
    let (_, catalog) = catalog_crafting_support::diagnostic_from(&database, None)?;
    assert_eq!(
        catalog
            .get("qualityDistributions")
            .ok_or("the distribution rows")?,
        &serde_json::json!([]),
        "no partial numeric row is emitted"
    );
    let diagnostics = catalog
        .pointer("/exportHealth/diagnostics")
        .and_then(Value::as_array)
        .ok_or("the diagnostics")?;
    assert!(
        diagnostics
            .iter()
            .any(
                |diagnostic| diagnostic["examples"].as_array().is_some_and(|examples| {
                    examples.iter().any(|example| {
                        example["field"] == field
                            && example["sourceType"] == "CraftingQualityDistributionNormal"
                    })
                })
            ),
        "the failure retains its source struct and exact field"
    );
    assert_quality_failure_health(&catalog)
}

fn assert_quality_failure_health(catalog: &Value) -> TestResult {
    let kinds = catalog
        .pointer("/exportHealth/kinds")
        .and_then(Value::as_array)
        .ok_or("the kind health")?;
    let quality = kinds
        .iter()
        .find(|kind| kind["kind"] == "qualityDistributions")
        .ok_or("the quality health")?;
    assert_eq!(
        quality
            .pointer("/sourceRecords/omittedAfterProjectionFailure")
            .ok_or("the projection-failure count")?,
        &serde_json::json!(1),
        "the failed projection is counted once"
    );
    assert_eq!(
        quality
            .pointer("/sourceRecords/readSuccessful")
            .ok_or("the successful-read count")?,
        &serde_json::json!(1),
        "its source read succeeded"
    );
    assert_eq!(
        catalog
            .pointer("/exportHealth/status")
            .and_then(Value::as_str)
            .ok_or("the export status")?,
        "incomplete",
        "numeric projection failure cannot be a complete export"
    );
    Ok(())
}

#[test]
fn a_float_minimum_is_never_coerced_even_when_it_is_a_whole_number() -> TestResult {
    for minimum in [1.0, 1.5] {
        assert_projection_failure(Numbers::SingleMinimum(minimum), "qualityDistribution.min")?;
    }
    Ok(())
}

#[test]
fn a_finite_double_beyond_the_f32_range_is_an_explicit_projection_failure() -> TestResult {
    assert_projection_failure(Numbers::Overflow, "qualityDistribution.stddev")
}

#[test]
fn a_nonfinite_parameter_cannot_become_an_undocumented_null() -> TestResult {
    assert_projection_failure(Numbers::NonFinite, "qualityDistribution.mean")
}
