// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The standing range and the reputation prerequisites of contracts in the P4KX catalog.

use crate::catalog_contracts::{A, B, SUB_ONE, TestResult, build, entry};

use serde_json::{Value, json};

#[test]
fn a_contract_lists_its_standing_range_and_the_requirements_of_every_level() -> TestResult {
    let (_, catalog) = build(None)?;
    let (low, high) = (
        "70000000-0000-4000-8000-000000000002",
        "70000000-0000-4000-8000-000000000001",
    );
    let requirement = |level: &str, exclude: bool, standings: Option<(&str, &str)>| {
        json!({
            "level": level,
            "exclude": exclude,
            "factionReputationGuid": "60000000-0000-4000-8000-000000000001",
            "scopeGuid": "60000000-0000-4000-8000-000000000002",
            "minStandingGuid": standings.map(|(minimum, _)| minimum),
            "maxStandingGuid": standings.map(|(_, maximum)| maximum),
        })
    };
    let inherited = [
        requirement("handler", false, None),
        requirement("contract", false, Some((low, high))),
    ];
    let contract = entry(&catalog, A, None)?;
    assert_eq!(contract["minStandingGuid"], low);
    assert_eq!(contract["maxStandingGuid"], high);
    assert_eq!(contract["standingRequirements"], json!(inherited));
    let sub_one = entry(&catalog, A, Some(SUB_ONE))?;
    assert_eq!(
        sub_one["minStandingGuid"], low,
        "a sub contract shares them"
    );
    assert_eq!(
        sub_one["standingRequirements"],
        json!([
            inherited[0],
            inherited[1],
            requirement("subContract", true, Some((high, low)))
        ]),
        "the sub contract's own requirement comes last"
    );
    let bare = entry(&catalog, B, None)?;
    assert_eq!(bare["minStandingGuid"], Value::Null);
    assert_eq!(bare["maxStandingGuid"], Value::Null);
    assert_eq!(
        bare["standingRequirements"],
        json!([requirement("handler", false, None)]),
        "only the handler's"
    );
    Ok(())
}
