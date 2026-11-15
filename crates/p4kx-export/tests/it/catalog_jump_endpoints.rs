// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Filter;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::health_origin_assertions as receipt;

#[path = "catalog_jump_endpoints_fixture.rs"]
mod fixture;
use fixture::{Scenario, build};

const ROWS: &str = "/placements/jumpEndpoints/rows";
const JUMP_TYPE: u128 = 101;
const ANOMALY_TYPE: u128 = 102;
const LIVE: u128 = 201;
const LOGICAL: u128 = 202;
const ANOMALY: u128 = 203;
const LOCAL: u128 = 301;
const OWNER: u128 = 401;
const CONFIGURATIONS: &str = "/configurations/jumpPointConfigurations/rows";

#[test]
fn jump_endpoint_occurrences_keep_same_position_and_do_not_pair_reversed_names() -> TestResult {
    let catalog = build(Some(&Filter::new("jump_")), Scenario::Operational)?;
    let rows = receipt::rows(&catalog, ROWS)?;
    let [first, second, logical, anomaly] = rows else {
        return Err(format!("expected four endpoint occurrences, got {}", rows.len()).into());
    };
    assert_ne!(
        receipt::at(first, "/instanceId/value")?,
        receipt::at(second, "/instanceId/value")?,
        "same place and record still have separate XML routes"
    );
    assert_eq!(
        receipt::at(first, "/worldPosition/value")?,
        receipt::at(second, "/worldPosition/value")?,
        "position equality has no effect on occurrence identity"
    );
    for row in [first, second, logical, anomaly] {
        receipt::cell(row, "declaredPartner", "absent")?;
    }
    assert_eq!(
        receipt::at(logical, "/gate/value/marker/value")?,
        &json!(false),
        "the source has a logical placement and a readable descriptor without a typed marker"
    );
    assert_eq!(
        receipt::at(anomaly, "/starmapType/value")?,
        "Anomaly",
        "a marked Anomaly is a proven gate occurrence"
    );
    assert_operational(&catalog, first)?;
    assert_operator(&catalog, first)?;
    assert_eq!(
        receipt::at(&catalog, "/projections/jumpConnections/availability")?,
        "absent",
        "all inspected endpoint documents were readable",
    );
    assert_eq!(
        receipt::rows(&catalog, "/projections/jumpConnections/rows")?,
        &[] as &[Value],
        "reverse names are not relationship evidence"
    );
    Ok(())
}

#[test]
fn skipped_routes_make_jump_inventory_unavailable_without_discarding_rows() -> TestResult {
    let filter = Filter::new("jump_");
    for (scenario, reason) in [
        (Scenario::ContainerCycle, "cycle"),
        (Scenario::InvalidAncestorPose, "invalid ancestor pose"),
        (Scenario::WalkDepth, "walk depth only"),
    ] {
        let catalog = build(Some(&filter), scenario)?;
        assert_eq!(
            receipt::at(&catalog, "/placements/jumpEndpoints/availability")?,
            "unavailable",
            "{reason} leaves endpoint scope incomplete"
        );
        assert_eq!(
            receipt::at(&catalog, "/projections/jumpConnections/availability")?,
            "unavailable",
            "{reason} cannot prove absence"
        );
        let endpoints = receipt::rows(&catalog, ROWS)?;
        assert_eq!(endpoints.len(), 4, "{reason} keeps every admitted endpoint");
        for row in endpoints {
            receipt::cell(row, "declaredPartner", "absent")?;
        }
    }
    Ok(())
}

#[test]
fn invalid_terminal_pose_still_keeps_complete_partner_evidence() -> TestResult {
    let catalog = build(Some(&Filter::new("jump_")), Scenario::InvalidEndpointPose)?;
    let endpoints = receipt::rows(&catalog, ROWS)?;
    let leaf = endpoints.last().ok_or("terminal endpoint")?;
    assert_eq!(
        endpoints.len(),
        5,
        "the invalid terminal transform does not omit its endpoint"
    );
    receipt::cell(leaf, "worldPosition", "invalid")?;
    receipt::cell(leaf, "declaredPartner", "absent")?;
    assert_eq!(
        receipt::at(&catalog, "/placements/jumpEndpoints/availability")?,
        "available",
        "there is no skipped declaration below the terminal endpoint"
    );
    assert_eq!(
        receipt::at(&catalog, "/projections/jumpConnections/availability")?,
        "absent",
        "the readable endpoint grammar may establish absence"
    );
    Ok(())
}

#[test]
fn jump_endpoint_local_duplicates_and_failed_reads_remain_explicit() -> TestResult {
    let duplicated = build(Some(&Filter::new("jump_live")), Scenario::Duplicate)?;
    let first = live_row(&duplicated)?;
    let [entity] = receipt::rows(first, "/gate/value/entities")? else {
        return Err("one metadata Entity remains".into());
    };
    assert_eq!(
        receipt::rows(entity, "/matches/value")?.len(),
        2,
        "every matching editor Object remains"
    );
    receipt::cell(entity, "object", "invalid")?;
    receipt::cell(entity, "operatingEntity", "invalid")?;
    receipt::cell(entity, "operatingConfigurations", "invalid")?;
    assert_eq!(
        receipt::rows(&duplicated, CONFIGURATIONS)?,
        &[] as &[Value],
        "an ambiguous XML owner cannot admit an out-of-filter configuration"
    );
    let unread = build(Some(&Filter::new("jump_live")), Scenario::BadCrc)?;
    let first = live_row(&unread)?;
    assert_eq!(
        receipt::at(first, "/gate/value/editor/state")?,
        "unreadable",
        "CRC failure never becomes a successful or absent source read"
    );
    receipt::cell(first, "declaredPartner", "unreadable")?;
    assert_eq!(
        receipt::at(&unread, "/projections/jumpConnections/availability")?,
        "unavailable",
        "an unreadable editor leaves partner inventory incomplete",
    );
    assert_eq!(
        receipt::at(&unread, "/placements/jumpEndpoints/availability")?,
        "unavailable",
        "the incomplete scope cannot certify its inventory"
    );
    Ok(())
}

#[test]
fn jump_metadata_rejects_two_admitted_locations_without_unscoped_fallback() -> TestResult {
    let catalog = build(Some(&Filter::new("jump_live")), Scenario::DuplicateMetadata)?;
    let endpoint = live_row(&catalog)?;
    let gate = receipt::at(endpoint, "/gate/value")?;
    receipt::cell(gate, "metadata", "invalid")?;
    receipt::cell(endpoint, "declaredPartner", "unreadable")?;
    assert_eq!(
        receipt::rows(gate, "/entities")?,
        &[] as &[Value],
        "neither an exact root nor exact stem-scoped copy wins ambiguity",
    );
    Ok(())
}

#[test]
fn duplicate_physical_jump_target_never_selects_one_gate() -> TestResult {
    let catalog = build(Some(&Filter::new("jump_live")), Scenario::DuplicateOuter)?;
    let row = live_row(&catalog)?;
    receipt::cell(row, "gate", "invalid")?;
    receipt::cell(row, "declaredPartner", "unreadable")?;
    let [first, second] = receipt::rows(row, "/targetCandidates/value")? else {
        return Err("both physical candidate entries are retained".into());
    };
    assert_ne!(
        receipt::at(first, "/value/entryIndex")?,
        receipt::at(second, "/value/entryIndex")?,
        "same normalized path still has distinct physical entries",
    );
    assert_eq!(
        receipt::at(&catalog, "/projections/jumpConnections/availability")?,
        "unavailable",
        "an ambiguous target cannot certify partner availability",
    );
    Ok(())
}

fn assert_operator(catalog: &Value, row: &Value) -> TestResult {
    let [entity] = receipt::rows(row, "/gate/value/entities")? else {
        return Err("expected the selected metadata entity".into());
    };
    let owner = receipt::cell(entity, "operatingEntity", "present")?;
    assert_eq!(
        receipt::at(owner, "/value/ownerGuid")?,
        &json!(common::guid(OWNER)),
        "use the exact class header and never the editor object name",
    );
    let uses = receipt::uses(catalog, owner)?;
    for field in [
        "@CryGUID",
        "@guid",
        "@type",
        "recordName",
        "sourceType",
        "guid",
        "recordIndex",
    ] {
        assert!(
            uses.iter()
                .any(|used| used.pointer("/location/field") == Some(&json!(field))),
            "operating entity must cite {field}",
        );
    }
    let configurations = receipt::cell(entity, "operatingConfigurations", "present")?;
    let [direct] = receipt::rows(configurations, "/value")? else {
        return Err("exactly one linked original component".into());
    };
    let [published] = receipt::rows(catalog, CONFIGURATIONS)? else {
        return Err("filtered selection must include its linked component owner".into());
    };
    assert_eq!(
        receipt::at(direct, "/ownerRecordIndex")?,
        receipt::at(published, "/ownerRecordIndex/value")?,
        "link targets the original row admitted as a dependency",
    );
    assert_eq!(
        receipt::at(direct, "/componentOrdinal")?,
        &json!(1),
        "ordinal is the original component position"
    );
    assert_eq!(
        receipt::at(direct, "/componentOrdinal")?,
        receipt::at(published, "/componentOrdinal/value")?,
        "link and published configuration use the same ordinal",
    );
    Ok(())
}

fn live_row(catalog: &Value) -> TestResult<&Value> {
    let rows = receipt::rows(catalog, ROWS)?;
    assert_eq!(
        rows.len(),
        2,
        "only the selected DCB Record's physical placements remain"
    );
    rows.first()
        .ok_or_else(|| "native endpoint placement".into())
}

fn assert_operational(catalog: &Value, row: &Value) -> TestResult {
    let [entity] = receipt::rows(row, "/gate/value/entities")? else {
        return Err("the original metadata Entity is retained".into());
    };
    receipt::present(entity, "cryGuid", &json!(common::guid(LOCAL)), Some(true))?;
    receipt::present(entity, "offsetFromPivot", &json!("0,1,2,3,4,5"), Some(true))?;
    let [object] = receipt::rows(entity, "/matches/value")? else {
        return Err("exact same-container editor Object expected".into());
    };
    receipt::present(object, "class", &json!("FixtureJump"), Some(true))?;
    let selection = receipt::cell(entity, "object", "present")?;
    let uses = receipt::uses(catalog, selection)?;
    assert!(
        uses.iter()
            .any(|used| used.pointer("/location/field") == Some(&json!("@CryGUID"))),
        "selection cites its exact source selector"
    );
    assert!(
        uses.iter()
            .any(|used| used.pointer("/location/field") == Some(&json!("@guid"))),
        "the selected target has its own physical source"
    );
    Ok(())
}
