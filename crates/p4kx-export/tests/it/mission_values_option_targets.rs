// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Authored race splits and faction condition targets.

use p4kx_export::CatalogPublicationMode;
use p4kx_testkit::{TestInstance, TestValue};
use serde_json::json;

use crate::mission_values_fixture::{self as fixture, Owner, TestResult};

#[test]
fn race_splits_and_faction_conditions_retain_single_values_and_target_identity() -> TestResult {
    let race = race();
    let organization = faction_conditions();
    let owner = Owner::Generator;
    let document = fixture::build(
        &fixture::database(
            vec![
                fixture::property("Race", race),
                fixture::property("Faction", organization),
            ],
            owner,
        ),
        CatalogPublicationMode::Normal,
    )?;
    assert_eq!(
        *fixture::field(
            fixture::generator_value(&document, "Race")?,
            "/targetSplits"
        )?,
        json!([{"targetSplit":0.125},{"targetSplit":0.0}]),
        "every authored split survives in order"
    );
    let conditions = fixture::field(
        fixture::generator_value(&document, "Faction")?,
        "/conditions",
    )?;
    assert_eq!(
        *fixture::field(conditions, "/0/condition")?,
        "factionIdentity",
        "FactionIdentityDef is a supported union variant"
    );
    assert_eq!(
        *fixture::field(conditions, "/0/factionIdentity/status")?,
        "dangling",
        "the faction target is not discarded when dangling"
    );
    assert_eq!(
        *fixture::field(conditions, "/1/rangeKm")?,
        0.375,
        "nearby range has Single precision"
    );
    assert_eq!(
        *fixture::field(conditions, "/2/rangeKm")?,
        1.125,
        "distant range has Single precision"
    );
    Ok(())
}

fn faction_conditions() -> TestInstance {
    let conditions = vec![
        fixture::object(
            TestInstance::new("DataSetMatchCondition_FactionIdentityDef")
                .value("factionIdentity", TestValue::Reference(fixture::MISSING)),
        ),
        fixture::object(
            TestInstance::new("DataSetMatchCondition_ExcludeNearbyLocationsDef")
                .value("exclusionRangeKm", TestValue::Single(0.375)),
        ),
        fixture::object(
            TestInstance::new("DataSetMatchCondition_ExcludeDistantLocationsDef")
                .value("exclusionRangeKm", TestValue::Single(1.125)),
        ),
    ];
    TestInstance::new("MissionPropertyValue_Organization")
        .value("matchConditions", fixture::array(conditions))
}

fn race() -> TestInstance {
    TestInstance::new("MissionPropertyValue_TimeTrialRace").value(
        "targetSplits",
        fixture::array(vec![
            fixture::object(
                TestInstance::new("TimeTrialSplit").value("targetSplit", TestValue::Single(0.125)),
            ),
            fixture::object(
                TestInstance::new("TimeTrialSplit").value("targetSplit", TestValue::Single(0.0)),
            ),
        ]),
    )
}
