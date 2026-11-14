// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::quality_v3_fixture_values as fixture;
use crate::quality_v3_origin_support::{self as origins, Domain, Use};
use crate::quality_v3_support as support;

#[test]
fn quality_v3_first_eligible_inverse_keeps_prior_and_winning_sources_without_later_payload()
-> TestResult {
    let (_, catalog) = support::diagnostic(&fixture::duplicate_quantizations())?;
    let winner = support::row(&catalog, support::INVERSE, 1)?;
    let members = support::at(winner, "/resourceTypeGuids")?;
    assert_eq!(
        membership(members)?,
        [
            support::guid(fixture::RESOURCE_B),
            support::guid(fixture::RESOURCE_A)
        ],
        "the provenance proof belongs to the established first eligible GUID result"
    );
    support::marker(members, Some(false))?;
    assert_eq!(
        support::at(members, "/sourceSetComplete")?,
        &json!(true),
        "all prior exclusions and the actual winning eligibility inputs are known"
    );
    let used = origins::uses(&catalog, members)?;
    for (record, field) in [
        (0, "qualityQuantization.bands"),
        (1, "qualityQuantization.bands"),
        (1, "qualityQuantization.bands[0].start"),
        (1, "qualityQuantization.bands[0].end"),
        (1, "qualityQuantization.bands[0].mappedValue"),
    ] {
        assert!(
            used.contains(&Use::new(Domain::Body, record, field, "selector")),
            "the earlier empty exclusion and the exact valid winning band contribute to first eligibility"
        );
    }
    assert!(
        used.contains(&Use::new(Domain::Header, 1, "guid", "selector")),
        "the actual winning nominal GUID selects this inverse group"
    );
    assert_no_later_or_unrelated_input(&used);
    Ok(())
}

fn assert_no_later_or_unrelated_input(used: &BTreeSet<Use>) {
    for role in ["payload", "selector", "conversion"] {
        for field in [
            "qualityQuantization.bands",
            "qualityQuantization.bands[0].start",
            "qualityQuantization.bands[0].end",
            "qualityQuantization.bands[0].mappedValue",
        ] {
            assert!(
                !used.contains(&Use::new(Domain::Body, 2, field, role)),
                "a later eligible same-GUID payload cannot taint the established first winner"
            );
        }
        assert!(
            !used.contains(&Use::new(Domain::Header, 2, "guid", role)),
            "later same-GUID identity does not establish the earlier winner"
        );
        for record in [3, 4] {
            assert!(
                !used.contains(&Use::new(Domain::Body, record, "properties[0].name", role)),
                "an unrelated loaded property name is not inverse membership evidence"
            );
        }
    }
}

fn membership(carrier: &Value) -> TestResult<Vec<String>> {
    support::at(carrier, "/value")?
        .as_array()
        .ok_or("inverse member list")?
        .iter()
        .map(|member| {
            support::state(member, "present")?;
            support::at(member, "/value")?
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "actual inverse GUID".into())
        })
        .collect()
}
