// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Raw catalog numbers and the independent finite interpreted crafting contract.

use std::error::Error;

use p4kx_export::CatalogError;
use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use crate::catalog_crafting_support;
use crate::quality_v3_support as source;

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
        source::at(
            &catalog,
            "/definitions/qualityDistributions/rows/0/qualityDistribution/value/payload/value/value/value/max/value/value"
        )?,
        &serde_json::json!(i32::MAX)
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
            Some(CatalogError::IncompleteExport(_))
        ),
        "invalid numeric data blocks normal publication"
    );
    let (_, catalog) = catalog_crafting_support::diagnostic_from(&database, None)?;
    assert_eq!(
        source::rows(&catalog, source::DISTRIBUTIONS)?.len(),
        1,
        "the raw owner remains present with its field-level contract error"
    );
    assert_eq!(
        source::at(
            source::row(&catalog, source::DISTRIBUTIONS, 0)?,
            "/qualityDistribution/value/payload/value/value/value/min/value/type"
        )?,
        &serde_json::json!("f32"),
        "wrong-width numeric storage remains an exact raw token"
    );
    let diagnostics = catalog
        .pointer("/exportHealth/diagnostics")
        .and_then(Value::as_array)
        .ok_or("the diagnostics")?;
    let mut found = false;
    for diagnostic in diagnostics {
        let examples = diagnostic["examples"].as_array().ok_or("source examples")?;
        for example in examples {
            found |= crate::health_origin_assertions::example_text(&catalog, example, "field")?
                == Some(&serde_json::json!(field))
                && crate::health_origin_assertions::example_text(&catalog, example, "sourceType")?
                    == Some(&serde_json::json!("CraftingQualityDistributionNormal"));
        }
    }
    assert!(
        found,
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
        &serde_json::json!(0),
        "a field contract failure does not omit the retained raw definition"
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
fn a_finite_double_beyond_f32_stays_exact_in_the_catalog_and_fails_the_interpreted_api()
-> TestResult {
    assert_interpreted_failure(Numbers::Overflow)?;
    let (_, catalog) = catalog_crafting_support::build_from(&database(Numbers::Overflow), None)?;
    let row = source::row(&catalog, source::DISTRIBUTIONS, 0)?;
    assert_eq!(
        source::at(
            row,
            "/qualityDistribution/value/payload/value/value/value/stddev/value"
        )?,
        &serde_json::json!({"type":"f64", "rawBits":"7e37e43c8800759c", "finite":1.0e300})
    );
    assert_eq!(
        source::at(&catalog, "/exportHealth/status")?,
        &serde_json::json!("complete")
    );
    Ok(())
}

#[test]
fn a_nonfinite_parameter_has_explicit_raw_bits_instead_of_an_undocumented_null() -> TestResult {
    assert_interpreted_failure(Numbers::NonFinite)?;
    let (_, catalog) = catalog_crafting_support::build_from(&database(Numbers::NonFinite), None)?;
    let row = source::row(&catalog, source::DISTRIBUTIONS, 0)?;
    assert_eq!(
        source::at(
            row,
            "/qualityDistribution/value/payload/value/value/value/mean/value"
        )?,
        &serde_json::json!({"type":"f32", "rawBits":format!("{:08x}", f32::NAN.to_bits()), "finite":null})
    );
    assert_eq!(
        source::at(&catalog, "/exportHealth/status")?,
        &serde_json::json!("complete")
    );
    Ok(())
}

fn assert_interpreted_failure(numbers: Numbers) -> TestResult {
    let bytes = database(numbers).build();
    let forge = DataForge::open(&bytes)?;
    let crafting = p4kx_export::read_crafting(&forge, None, None)?;
    assert!(
        crafting.quality_distributions.is_empty(),
        "the current interpreted API keeps its finite f32 contract"
    );
    assert_eq!(
        crafting.failures.len(),
        1,
        "the original interpreted field failure remains visible"
    );
    Ok(())
}
