// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::{CatalogPublicationMode, Filter, Kind};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{DcbOrigin, TestResult};
use crate::quality_v3_fixture_values as values;
use crate::quality_v3_origin_support::{self as origins, Domain, Use};
use crate::quality_v3_support as support;

#[test]
fn quality_v3_retains_record_reference_forms_without_reading_a_target_interpretation() -> TestResult
{
    let database = values::reference_forms();
    let (_, catalog) = support::normal(&database)?;
    for (dataset, owner, field, source_field, target, guid) in [
        (
            support::DISTRIBUTIONS,
            0,
            "qualityDistribution",
            "qualityDistributionRecord",
            1,
            0x53,
        ),
        (
            support::DISTRIBUTIONS,
            2,
            "locationOverride",
            "locationOverrideRecord",
            3,
            0x55,
        ),
        (
            support::QUANTIZATIONS,
            4,
            "qualityQuantization",
            "qualityQuantizationRecord",
            5,
            0x56,
        ),
    ] {
        let row = support::row(&catalog, dataset, owner)?;
        let payload = support::at(row, &format!("/{field}/value/payload/value"))?;
        assert_eq!(support::at(payload, "/type")?, &json!("recordRef"));
        let reference = support::at(payload, "/reference")?;
        support::state(reference, "present")?;
        assert_eq!(
            support::at(reference, "/value/guid/value")?,
            &json!(support::guid(guid))
        );
        assert_eq!(
            support::at(reference, "/value/targetRecordIndex/value")?,
            &json!(target)
        );
        let authored = format!("{field}.{source_field}");
        assert_eq!(
            origins::uses(&catalog, support::at(reference, "/value/guid")?)?,
            BTreeSet::from([Use::new(Domain::Body, owner, &authored, "payload")])
        );
        assert_target_headers(&catalog, reference, target)?;
    }
    let base = support::row(&catalog, support::DISTRIBUTIONS, 3)?;
    assert_eq!(
        support::at(base, "/locationOverride/value/payload/value/type")?,
        &json!("emptyBase")
    );
    assert_eq!(
        support::at(&catalog, "/exportHealth/status")?,
        &json!("complete"),
        "raw reference cycles and known fieldless forms do not require a removed inline interpretation"
    );
    Ok(())
}

#[test]
fn quality_v3_keeps_duplicate_physical_records_and_the_first_eligible_guid_inverse_policy()
-> TestResult {
    let (built, catalog) = support::diagnostic(&values::duplicate_quantizations())?;
    let definitions = support::rows(&catalog, support::QUANTIZATIONS)?;
    assert_eq!(
        definitions.len(),
        3,
        "ineligible and repeated physical definitions remain raw facts"
    );
    for index in 0..3 {
        let row = support::row(&catalog, support::QUANTIZATIONS, index)?;
        assert_eq!(
            support::at(row, "/guid/value")?,
            &json!(support::guid(values::QUANTIZATION))
        );
        assert_eq!(
            support::at(row, "/recordPath/value")?,
            &json!("libs/foundry/records/crafting/qualityquantization/shared")
        );
    }
    for index in [0, 2] {
        assert_eq!(
            support::at(
                support::row(&catalog, support::INVERSE, index)?,
                "/resourceTypeGuids/value"
            )?,
            &json!([]),
            "only the first eligible physical occurrence receives nominal-GUID inverse links"
        );
    }
    let winner = support::row(&catalog, support::INVERSE, 1)?;
    assert_eq!(
        membership(winner)?,
        [
            support::guid(values::RESOURCE_B),
            support::guid(values::RESOURCE_A)
        ]
    );
    assert_eq!(
        built
            .summary()
            .kind(Kind::QualityQuantizations)
            .map(|kind| kind.count),
        Some(3)
    );
    assert_eq!(
        built.summary().quantizations().unused,
        1,
        "unused retains its eligible-quantization scope"
    );
    assert_eq!(built.summary().quantizations().unresolved_links, 0);
    let resource = support::row(&catalog, "definitions/resourceTypes", 3)?;
    let raw = support::at(
        resource,
        "/properties/value/1/value/payload/value/qualityQuantization/value/payload/value/reference/value/targetRecordIndex/value",
    )?;
    assert_eq!(
        raw,
        &json!(0),
        "the physical raw Reference target is independent of the first eligible nominal-GUID projection"
    );
    Ok(())
}

#[test]
fn quality_v3_inverse_keeps_actual_positive_negative_and_filtered_dependency_witnesses()
-> TestResult {
    let database = values::inverse_dependencies();
    let filter = Filter::new("*/quantizer");
    let (_, catalog) = support::build(
        &database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        Some(&filter),
    )?;
    assert_eq!(
        support::rows(&catalog, "definitions/resourceTypes")?.len(),
        0,
        "dependency reads do not publish filtered definitions"
    );
    let inverse = support::row(&catalog, support::INVERSE, 0)?;
    assert_eq!(
        membership(inverse)?,
        [
            support::guid(values::RESOURCE_B),
            support::guid(values::RESOURCE_A)
        ]
    );
    let carrier = support::at(inverse, "/resourceTypeGuids")?;
    support::marker(carrier, Some(false))?;
    assert_eq!(support::at(carrier, "/sourceSetComplete")?, &json!(true));
    let uses = origins::uses(&catalog, carrier)?;
    for (record, field) in [
        (
            1,
            "properties[1].qualityQuantization.qualityQuantizationRecord",
        ),
        (
            2,
            "properties[0].qualityQuantization.qualityQuantizationRecord",
        ),
        (
            3,
            "properties[0].qualityQuantization.qualityQuantizationRecord",
        ),
    ] {
        assert!(
            uses.contains(&Use::new(Domain::Body, record, field, "selector")),
            "the complete inverse retains the actual positive or excluding declaration witness"
        );
    }
    let health = support::kind_health(&catalog, "qualityQuantizations")?;
    assert_eq!(support::at(health, "/sourceRecords/selected")?, &json!(1));
    assert_eq!(
        support::at(health, "/sourceRecords/dependencies")?,
        &json!(3)
    );
    assert_eq!(
        support::at(&catalog, "/exportHealth/status")?,
        &json!("complete")
    );
    Ok(())
}

fn assert_target_headers(catalog: &Value, reference: &Value, target: u64) -> TestResult {
    for (cell, header, role) in [
        ("targetRecordIndex", "recordIndex", "selector"),
        ("targetType", "sourceType", "selector"),
        ("recordPath", "fileName", "conversion"),
    ] {
        assert_eq!(
            origins::uses(catalog, support::at(reference, &format!("/value/{cell}"))?)?,
            BTreeSet::from([Use::new(Domain::Header, target, header, role)]),
            "reference target headers do not claim target payload contributions"
        );
    }
    Ok(())
}

fn membership(row: &Value) -> TestResult<Vec<String>> {
    let members = support::at(row, "/resourceTypeGuids/value")?
        .as_array()
        .ok_or("inverse membership")?;
    let mut result = Vec::new();
    for member in members {
        support::state(member, "present")?;
        result.push(
            support::at(member, "/value")?
                .as_str()
                .ok_or("resource GUID")?
                .to_owned(),
        );
    }
    Ok(result)
}
