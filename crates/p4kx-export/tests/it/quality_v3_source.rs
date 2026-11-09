// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::TestValue;
use serde_json::json;

use crate::commodity_v3_fixture::TestResult;
use crate::quality_v3_fixture_schema as schema;
use crate::quality_v3_fixture_values as values;
use crate::quality_v3_support as support;

#[test]
fn quality_v3_publishes_complete_source_tables_and_only_a_genuine_inverse_projection() -> TestResult
{
    let (built, catalog) = support::normal(&values::healthy())?;
    assert_eq!(support::at(&catalog, "/schemaVersion")?, &json!(3));
    assert_eq!(support::rows(&catalog, support::DISTRIBUTIONS)?.len(), 1);
    assert_eq!(support::rows(&catalog, support::QUANTIZATIONS)?.len(), 1);
    let inverse = support::rows(&catalog, support::INVERSE)?;
    assert_eq!(inverse.len(), 1);
    for field in ["qualityDistributions", "qualityQuantizations"] {
        assert!(
            catalog.get(field).is_none(),
            "v3 emits one canonical source owner, not a second flat compatibility payload"
        );
    }
    for path in [
        "/projections/qualityDistributionInterpretations",
        "/projections/qualityQuantizationInterpretations",
    ] {
        assert!(
            catalog.pointer(path).is_none(),
            "full reduced quality mirrors have no independent consumer"
        );
    }
    for row in inverse {
        assert!(
            row.get("bands").is_none() && row.get("distribution").is_none(),
            "the inverse relation does not copy authored quality payloads"
        );
    }
    assert_eq!(
        built
            .summary()
            .kind(Kind::QualityDistributions)
            .map(|kind| (kind.count, kind.skipped)),
        Some((1, 0))
    );
    assert_eq!(
        built
            .summary()
            .kind(Kind::QualityQuantizations)
            .map(|kind| (kind.count, kind.skipped)),
        Some((1, 0))
    );
    assert_eq!(
        support::at(&catalog, "/exportHealth/status")?,
        &json!("complete")
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "the actual canonical output validates against its public v3 schema"
    );
    Ok(())
}

#[test]
fn quality_v3_distinguishes_absent_null_and_present_empty_source_declarations() -> TestResult {
    let (_, absent) = support::normal(&values::absent_distribution())?;
    let row = support::row(&absent, support::DISTRIBUTIONS, 0)?;
    support::state(support::at(row, "/qualityDistribution")?, "absent")?;
    let database = schema::schema().record(values::distribution(
        values::DISTRIBUTION,
        "null",
        TestValue::Null,
    ));
    let (_, null) = support::normal(&database)?;
    let row = support::row(&null, support::DISTRIBUTIONS, 0)?;
    support::state(support::at(row, "/qualityDistribution")?, "null")?;
    let database = schema::schema().record(values::override_record(
        values::DISTRIBUTION,
        "empty_overrides",
        values::overrides(Vec::new()),
    ));
    let (_, overrides) = support::normal(&database)?;
    let row = support::row(&overrides, support::DISTRIBUTIONS, 0)?;
    let list = support::at(
        row,
        "/locationOverride/value/payload/value/value/value/locationOverrideList",
    )?;
    support::state(list, "present")?;
    assert_eq!(support::at(list, "/value")?, &json!([]));
    let database = schema::schema().record(values::quantization(
        values::QUANTIZATION,
        "Empty",
        "empty",
        values::bands(Vec::new()),
    ));
    let (_, empty) = support::normal(&database)?;
    let row = support::row(&empty, support::QUANTIZATIONS, 0)?;
    let bands = support::at(
        row,
        "/qualityQuantization/value/payload/value/value/value/bands",
    )?;
    support::state(bands, "present")?;
    assert_eq!(support::at(bands, "/value")?, &json!([]));
    assert_eq!(
        support::at(
            support::row(&empty, support::INVERSE, 0)?,
            "/resourceTypeGuids/value"
        )?,
        &json!([])
    );
    assert_eq!(
        support::at(&empty, "/exportHealth/status")?,
        &json!("complete"),
        "fully represented empty source data is distinct from an unavailable inverse computation"
    );
    Ok(())
}

#[test]
fn quality_v3_keeps_nullable_override_positions_repeats_and_exact_inline_values() -> TestResult {
    let database = schema::schema().record(values::override_record(
        values::DISTRIBUTION,
        "nullable_overrides",
        values::overrides(vec![
            values::override_entry(17),
            TestValue::Null,
            values::override_entry(17),
        ]),
    ));
    let (_, catalog) = support::normal(&database)?;
    let row = support::row(&catalog, support::DISTRIBUTIONS, 0)?;
    let list = support::at(
        row,
        "/locationOverride/value/payload/value/value/value/locationOverrideList/value",
    )?
    .as_array()
    .ok_or("original override list")?;
    assert_eq!(
        list.len(),
        3,
        "nullable and repeated authored positions cannot be compacted"
    );
    support::state(list.get(1).ok_or("null slot")?, "null")?;
    for index in [0, 2] {
        let entry = list.get(index).ok_or("retained override position")?;
        support::state(entry, "present")?;
        let minimum = support::at(
            entry,
            "/value/payload/value/qualityDistribution/value/payload/value/value/value/min/value",
        )?;
        assert_eq!(minimum, &json!({"type": "int32", "value": 17}));
    }
    assert_eq!(
        support::at(&catalog, "/exportHealth/status")?,
        &json!("complete")
    );
    Ok(())
}

#[test]
fn quality_v3_retains_all_ordered_bands_when_a_configured_interval_is_reversed() -> TestResult {
    let database = schema::schema().record(values::quantization(
        values::QUANTIZATION,
        "Reversed",
        "reversed",
        values::bands(vec![
            values::band(0, 9, 3),
            values::band(50, 40, 4),
            values::band(0, 9, 3),
        ]),
    ));
    support::assert_normal_refusal(&database)?;
    let (built, catalog) = support::diagnostic(&database)?;
    let row = support::row(&catalog, support::QUANTIZATIONS, 0)?;
    let bands = support::at(row, support::BANDS)?
        .as_array()
        .ok_or("authored bands")?;
    let mut observed = Vec::new();
    for band in bands {
        let start = support::at(band, "/value/start/value/value")?
            .as_i64()
            .ok_or("source start")?;
        let end = support::at(band, "/value/end/value/value")?
            .as_i64()
            .ok_or("source end")?;
        let mapped = support::at(band, "/value/mappedValue/value/value")?
            .as_i64()
            .ok_or("source mapped value")?;
        observed.push((start, end, mapped));
    }
    assert_eq!(observed, [(0, 9, 3), (50, 40, 4), (0, 9, 3)]);
    support::assert_field_error(
        &catalog,
        "qualityQuantization.bands[1].end",
        "CraftingQualityQuantizationBand",
    )?;
    let health = support::kind_health(&catalog, "qualityQuantizations")?;
    assert_eq!(
        support::at(health, "/sourceRecords/readSuccessful")?,
        &json!(1)
    );
    assert_eq!(support::at(health, "/sourceRecords/projected")?, &json!(1));
    assert_eq!(
        support::at(health, "/sourceRecords/omittedAfterProjectionFailure")?,
        &json!(0)
    );
    assert_eq!(
        built
            .summary()
            .kind(Kind::QualityQuantizations)
            .map(|kind| kind.count),
        Some(1),
        "a retained physical definition is counted even when its configured interval is invalid"
    );
    assert_eq!(
        support::at(&catalog, "/exportHealth/status")?,
        &json!("incomplete")
    );
    Ok(())
}
