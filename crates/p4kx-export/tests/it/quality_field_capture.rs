// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::quality_v3_support as support;

#[path = "quality_field_capture_assertions.rs"]
mod assertions;
#[path = "quality_field_capture_source.rs"]
mod source;
use source::Case;

#[test]
fn quality_capture_a_genuine_valid_counterpart_publishes_normally() -> TestResult {
    let database = source::database(Case::Healthy);
    source::original(&database, Case::Healthy)?;
    let (built, catalog) = support::normal(&database)?;
    assert!(
        built.summary().is_complete(),
        "the actual well-typed source passes the same public pipeline"
    );
    assert_eq!(
        catalog.pointer("/exportHealth/status"),
        Some(&json!("complete")),
        "the valid source is a positive control"
    );
    Ok(())
}

#[test]
fn quality_capture_actual_wrong_width_has_a_genuine_public_field_body() -> TestResult {
    let database = source::database(Case::Width);
    source::original(&database, Case::Width)?;
    let (_, catalog) = support::diagnostic(&database)?;
    assertions::example(
        &catalog,
        "malformedSource",
        "qualityDistribution.min",
        "CraftingQualityDistributionNormal",
    )?;
    let row = support::row(&catalog, support::DISTRIBUTIONS, 0)?;
    assert_eq!(
        support::at(row, &format!("{}/min/value", support::CURVE))?,
        &json!({"type":"uint64","value":"18446744073709551615"}),
        "the guard recipe cannot alter or narrow the actual original token"
    );
    support::assert_normal_refusal(&database)
}

#[test]
fn quality_capture_reversed_interval_retains_every_slot_and_its_real_guard_body() -> TestResult {
    let database = source::database(Case::Reversed);
    source::original(&database, Case::Reversed)?;
    let (_, catalog) = support::diagnostic(&database)?;
    assertions::example(
        &catalog,
        "malformedSource",
        "qualityQuantization.bands[1].end",
        "CraftingQualityQuantizationBand",
    )?;
    let row = support::row(&catalog, support::QUANTIZATIONS, 0)?;
    let bands = support::at(row, support::BANDS)?
        .as_array()
        .ok_or("the original ordered retained bands")?;
    let actual: Vec<_> = bands
        .iter()
        .map(|band| -> TestResult<_> {
            Ok((
                support::at(band, "/value/start/value/value")?.clone(),
                support::at(band, "/value/end/value/value")?.clone(),
                support::at(band, "/value/mappedValue/value/value")?.clone(),
            ))
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(
        actual,
        [
            (json!(0), json!(9), json!(3)),
            (json!(50), json!(40), json!(4)),
            (json!(0), json!(9), json!(3))
        ],
        "order, repeats and original values remain intact"
    );
    support::assert_normal_refusal(&database)
}

#[test]
fn quality_capture_required_absent_declaration_uses_real_negative_source_evidence() -> TestResult {
    let database = source::database(Case::Missing);
    source::original(&database, Case::Missing)?;
    let (_, catalog) = support::diagnostic(&database)?;
    let example = assertions::example(
        &catalog,
        "malformedSource",
        "qualityQuantization.bands[0].start",
        "CraftingQualityQuantizationBand",
    )?;
    let row = support::row(&catalog, support::QUANTIZATIONS, 0)?;
    let band = support::at(row, support::BANDS)?
        .as_array()
        .and_then(|bands| bands.first())
        .ok_or("one actual retained band")?;
    support::state(support::at(band, "/value/start")?, "absent")?;
    assert_eq!(
        support::at(band, "/value/end/value/value")?,
        &json!(9),
        "the existing readable neighbor survives"
    );
    let field = crate::health_origin_assertions::cell(example, "field", "present")?;
    let uses = crate::health_origin_assertions::uses(&catalog, field)?;
    assert!(
        uses.iter().any(
            |used| used.pointer("/location/namespace").and_then(Value::as_str)
                == Some("dataforgeValue")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$fields")
                && used.get("role").and_then(Value::as_str) == Some("selector")
        ),
        "the absence recipe consumes the actual complete declaration survey instead of a fabricated value"
    );
    support::assert_normal_refusal(&database)
}

#[test]
fn quality_capture_unsupported_resource_property_keeps_its_real_nested_type_and_path() -> TestResult
{
    let database = source::database(Case::Unknown);
    source::original(&database, Case::Unknown)?;
    let (_, catalog) = support::diagnostic(&database)?;
    assertions::example(
        &catalog,
        "unsupportedValue",
        "properties[0]",
        "FixtureFutureProperty",
    )?;
    let row = support::row(&catalog, "definitions/resourceTypes", 0)?;
    let property = support::at(row, "/properties/value/0/value")?;
    support::state(support::at(property, "/payload")?, "unsupported")?;
    assert_eq!(
        support::at(property, "/name/value")?,
        &json!("future"),
        "the known source neighbor remains available"
    );
    support::assert_normal_refusal(&database)
}
