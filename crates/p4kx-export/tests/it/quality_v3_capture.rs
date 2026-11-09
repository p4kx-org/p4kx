// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::{Filter, Kind};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::quality_v3_capture_fixture as fixture;
use crate::quality_v3_capture_support::{self as capture, CapturedFixture};
use crate::quality_v3_origin_support::{self as origins, Domain, Use};
use crate::quality_v3_support as support;

#[test]
fn quality_v3_filtered_dependency_and_same_guid_alias_cannot_excuse_the_original_number()
-> TestResult {
    for alias in [false, true] {
        dependency_case(alias)?;
    }
    Ok(())
}

fn dependency_case(alias: bool) -> TestResult {
    let original = CapturedFixture::new(&fixture::filtered_dependency(alias))?;
    original.resource_mean(1)?;
    if alias {
        original.resource_mean(2)?;
    }
    let filter = Filter::new("included");
    let (built, catalog) = original.refused_then_diagnostic(Some(&filter))?;
    support::row(&catalog, support::QUANTIZATIONS, 0)?;
    support::row(&catalog, support::INVERSE, 0)?;
    capture::assert_projected(&catalog, "qualityQuantizations")?;
    assert_eq!(
        built
            .summary()
            .kind(Kind::QualityQuantizations)
            .map(|kind| kind.count),
        Some(1),
        "the retained quantizer remains one selected definition"
    );
    capture::assert_uncovered(
        &catalog,
        "qualityQuantizations",
        "dependency",
        1,
        fixture::RESOURCE_MEAN,
    )?;
    assert_resource_rows(&catalog, alias)?;
    Ok(())
}

fn assert_resource_rows(catalog: &Value, alias: bool) -> TestResult {
    let rows = support::rows(catalog, "definitions/resourceTypes")?;
    let indices = rows
        .iter()
        .map(|row| {
            support::at(row, "/recordIndex/value")?
                .as_u64()
                .ok_or_else(|| "physical ResourceType index".into())
        })
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(
        indices,
        if alias { vec![2] } else { Vec::new() },
        "required dependency reads never publish the filtered original definition"
    );
    if alias {
        let row = support::row(catalog, "definitions/resourceTypes", 2)?;
        let mean = support::at(
            row,
            "/properties/value/0/value/payload/value/qualityDistribution/value/payload/value/value/value/mean",
        )?;
        capture::assert_exported_mean(mean)?;
        assert_eq!(
            origins::uses(catalog, mean)?,
            BTreeSet::from([Use::new(Domain::Body, 2, fixture::RESOURCE_MEAN, "payload")]),
            "the emitted same-GUID alias has its own original token, not authority for Record 1"
        );
    }
    Ok(())
}

#[test]
fn quality_v3_an_emitted_owner_does_not_cover_an_unexported_numeric_field() -> TestResult {
    let original = CapturedFixture::new(&fixture::uncaptured_owner())?;
    original.spare_mean()?;
    let (built, catalog) = original.refused_then_diagnostic(None)?;
    let row = support::row(&catalog, support::DISTRIBUTIONS, 0)?;
    assert!(
        row.get("spareCurve").is_none(),
        "an unsupported extra field is diagnosed rather than invented as a generic payload"
    );
    let mean = support::at(row, &format!("{}/mean", support::CURVE))?;
    capture::assert_exported_mean(mean)?;
    assert_eq!(
        origins::uses(&catalog, mean)?,
        BTreeSet::from([Use::new(
            Domain::Body,
            0,
            "qualityDistribution.mean",
            "payload"
        )]),
        "same owner/type/bits do not conflate distinct original numeric fields"
    );
    capture::assert_uncovered(
        &catalog,
        "qualityDistributions",
        "selected",
        0,
        "spareCurve.mean",
    )?;
    capture::assert_projected(&catalog, "qualityDistributions")?;
    assert_eq!(
        built
            .summary()
            .kind(Kind::QualityDistributions)
            .map(|kind| kind.count),
        Some(1),
        "the complete retained part of the owner remains counted"
    );
    assert_eq!(
        support::at(mean, "/sourceSetComplete")?,
        &json!(true),
        "the independently captured number keeps its complete provenance"
    );
    Ok(())
}
