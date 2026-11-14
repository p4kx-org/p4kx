// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::Filter;
use serde_json::Value;

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::quality_origin_wide_fixture::{self as source, WIDE};
use crate::quality_v3_fixture_values as values;
use crate::quality_v3_support as support;

#[test]
fn wide_inverse_member_keeps_every_actual_relationship_and_later_owner() -> TestResult {
    let filter = Filter::new("*/quantizer");
    let (catalog, source) = source::observed(WIDE, Some(&filter), DcbOrigin::Encrypted)?;
    assert!(
        support::rows(&catalog, "definitions/resourceTypes")?.is_empty(),
        "the two resources are actual inverse dependencies outside the output Filter"
    );
    let (list, wide, narrow) = members(&catalog, source.quantizer)?;
    assert_eq!(
        source::relationships(&catalog, narrow)?,
        BTreeSet::from([(source.narrow, 0)]),
        "an independently selected member keeps its own original relationship after the wide predecessor"
    );
    assert_eq!(narrow["sourceSetComplete"], true);
    let expected: BTreeSet<_> = (0..source.wide_count)
        .map(|index| (source.wide, index))
        .collect();
    let actual = source::relationships(&catalog, wide)?;
    assert_eq!(
        actual.len(),
        expected.len(),
        "every authored repeated Reference has one exact contributing location"
    );
    assert!(
        actual == expected,
        "the wide member's selectors stay on its own physical Record"
    );
    let all = source::relationships(&catalog, list)?;
    assert_eq!(
        all.len(),
        expected.len() + 1,
        "the inverse array retains both original resource witnesses"
    );
    assert!(all.contains(&(source.narrow, 0)));
    for cell in [list, wide, narrow] {
        support::marker(cell, Some(true))?;
        assert_eq!(cell["sourceSetComplete"], true);
    }
    Ok(())
}

#[test]
fn commodity_fallback_uses_only_its_actual_quantizer_member() -> TestResult {
    let (catalog, source) = source::observed(2, None, DcbOrigin::Plain)?;
    let (_, wide, narrow) = members(&catalog, source.quantizer)?;
    let wide_expected = BTreeSet::from([(source.wide, 0), (source.wide, 1)]);
    let narrow_expected = BTreeSet::from([(source.narrow, 0)]);
    assert_eq!(source::relationships(&catalog, wide)?, wide_expected);
    assert_eq!(source::relationships(&catalog, narrow)?, narrow_expected);
    let actual = commodity(&catalog, values::RESOURCE_B)?;
    assert_eq!(actual["sourceSetComplete"], true);
    support::marker(actual, Some(false))?;
    assert_eq!(
        source::relationships(&catalog, actual)?,
        narrow_expected,
        "the fallback consumes the actual matching membership instead of the aggregate inverse"
    );
    Ok(())
}

#[test]
fn wide_commodity_fallback_keeps_its_complete_relationship_proof() -> TestResult {
    let (catalog, source) = source::observed(WIDE, None, DcbOrigin::Encrypted)?;
    let actual = commodity(&catalog, values::RESOURCE_A)?;
    assert_eq!(
        actual["sourceSetComplete"], true,
        "the complete original source cannot lose provenance to a fixed capture quota"
    );
    support::marker(actual, Some(true))?;
    let expected: BTreeSet<_> = (0..source.wide_count)
        .map(|index| (source.wide, index))
        .collect();
    let retained = source::relationships(&catalog, actual)?;
    assert_eq!(retained.len(), expected.len());
    assert_eq!(
        retained, expected,
        "each wide Commodity selector refers to an actual relationship"
    );
    Ok(())
}

fn members(catalog: &Value, quantizer: u32) -> TestResult<(&Value, &Value, &Value)> {
    let inverse = support::row(catalog, support::INVERSE, u64::from(quantizer))?;
    let list = support::at(inverse, "/resourceTypeGuids")?;
    let members = list["value"].as_array().ok_or("actual inverse members")?;
    let [narrow, wide] = members.as_slice() else {
        return Err("both source members".into());
    };
    assert_eq!(
        narrow["value"],
        common::guid(values::RESOURCE_B),
        "preserve GUID sort"
    );
    assert_eq!(
        wide["value"],
        common::guid(values::RESOURCE_A),
        "the wide member keeps its physical resource GUID"
    );
    Ok((list, wide, narrow))
}

fn commodity(catalog: &Value, guid: u128) -> TestResult<&Value> {
    let row = common::definition(catalog, guid)?;
    let selection = support::at(row, "/selection")?;
    assert_eq!(
        selection["state"], "present",
        "the Commodity fallback has a selected value"
    );
    assert_eq!(
        selection["value"], "resourceFallback",
        "the Commodity selected its original resource fallback"
    );
    Ok(selection)
}
