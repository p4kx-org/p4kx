// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, Filter, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_resource_network_fixture.rs"]
mod fixture;

const ROWS: &str = "/configurations/resourceNetworkParameters/rows";

#[test]
fn each_mode_owns_its_original_rules_with_per_field_origin() -> TestResult {
    let database = fixture::database(fixture::Layout::Complete);
    let schema = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        schema
            .validate(&catalog)
            .map_err(|error| format!("catalog schema: {error}"))?;
        assert_eq!(
            catalog["exportHealth"]["status"], "complete",
            "unrelated mode fields stay outside the resource health scope"
        );
        let [first, second, other, absent] = proof::rows(&catalog, ROWS)? else {
            return Err("one physically owned row per mode".into());
        };
        check_owner(first, fixture::FIRST, 0, "FixtureFirst", encrypted)?;
        check_owner(second, fixture::SECOND, 1, "FixtureSecond", encrypted)?;
        check_rules(first, true, encrypted)?;
        check_rules(second, false, encrypted)?;
        check_owner(other, fixture::OTHER, 2, "FixtureOther", encrypted)?;
        proof::cell(
            proof::at(other, "/params/value")?,
            "resourceNetworkParams",
            "absent",
        )?;
        check_owner(absent, fixture::NULL, 3, "FixtureNull", encrypted)?;
        proof::cell(absent, "params", "null")?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn global_scope_respects_record_filter() -> TestResult {
    let filter = Filter::new("*FixtureFirst");
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Layout::Complete),
        DcbOrigin::Plain,
    )?;
    let options = CatalogOptions {
        filter: Some(&filter),
        ..common::options()
    };
    let filtered: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let [selected] = proof::rows(&filtered, ROWS)? else {
        return Err("exactly the selected resource-rule owner".into());
    };
    check_owner(selected, fixture::FIRST, 0, "FixtureFirst", false)?;
    let toggle = proof::at(
        selected,
        "/params/value/resourceNetworkParams/value/enableItemResourceNetwork",
    )?;
    let used = proof::uses(&filtered, toggle)?;
    let mut body = used
        .iter()
        .filter(|source| {
            source
                .pointer("/location/namespace")
                .and_then(Value::as_str)
                == Some("dataforgeValue")
        })
        .peekable();
    assert!(
        body.peek().is_some(),
        "the selected flag has its physical body evidence"
    );
    assert!(
        body.all(|source| {
            source
                .pointer("/location/indices/0")
                .and_then(Value::as_str)
                == Some("0")
        }),
        "filtered modes do not contribute to the selected flag"
    );

    Ok(())
}

#[test]
fn malformed_global_rule_affects_export_health() -> TestResult {
    let (_folder, malformed) = common::open(
        &fixture::database(fixture::Layout::Invalid),
        DcbOrigin::Plain,
    )?;
    assert!(
        matches!(
            build_catalog(&malformed, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "a wrong original declaration must affect publication"
    );
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let report: Value = serde_json::from_slice(build_catalog(&malformed, &options)?.json())?;
    let rows = proof::rows(&report, ROWS)?;
    let [invalid, ..] = rows else {
        return Err("first malformed mode".into());
    };
    proof::cell(
        proof::at(invalid, "/params/value")?,
        "resourceNetworkParams",
        "invalid",
    )?;
    proof::closure(&report)?;
    Ok(())
}

fn check_owner(row: &Value, guid: u128, index: u32, id: &str, encrypted: bool) -> TestResult {
    let owner = proof::at(row, "/source")?;
    proof::present(owner, "guid", &json!(common::guid(guid)), Some(encrypted))?;
    proof::present(owner, "recordIndex", &json!(index), Some(encrypted))?;
    proof::present(owner, "sourceType", &json!("GameMode"), Some(encrypted))?;
    proof::present(row, "gameModeId", &json!(id), Some(encrypted))?;
    Ok(())
}

fn check_rules(row: &Value, enabled: bool, encrypted: bool) -> TestResult {
    let params = proof::at(row, "/params/value")?;
    proof::present(
        params,
        "sourceType",
        &json!("SStandardParams"),
        Some(encrypted),
    )?;
    let rules = proof::at(params, "/resourceNetworkParams/value")?;
    proof::present(
        rules,
        "sourceType",
        &json!("ResourceNetworkParams"),
        Some(encrypted),
    )?;
    for (field, value) in [
        ("enableItemResourceNetwork", enabled),
        ("enableSalvageItemRepair", false),
        ("enableMisfires", true),
        ("enableCriticalMisfires", false),
        ("doorsIgnoreRN", true),
        ("enableExperimentalArmorPenetration", false),
        ("shipPassDestroyedPartDamageToComponent", enabled),
        ("shipPassDestroyedPartDamageToChild", !enabled),
    ] {
        proof::present(rules, field, &json!(value), Some(encrypted))?;
    }
    proof::present(
        rules,
        "shipDestructionBufferMultiplier",
        &json!({
            "rawBits":"80000000", "finite":-0.0
        }),
        Some(encrypted),
    )?;
    proof::present(
        rules,
        "shipDestructionHealthThreshold",
        &json!({
            "rawBits":format!("{:08x}", fixture::THRESHOLD_BITS), "finite":null
        }),
        Some(encrypted),
    )?;
    check_future_rule(rules, encrypted)
}

fn check_future_rule(rules: &Value, encrypted: bool) -> TestResult {
    let [extra] = proof::rows(rules, "/additionalFields/roots")? else {
        return Err("the original future rule".into());
    };
    proof::present(extra, "name", &json!("fixtureRawRule"), Some(encrypted))?;
    let index = proof::at(extra, "/value/valueIndex")?
        .as_str()
        .ok_or("original rule arena index")?
        .parse::<usize>()?;
    let original = proof::rows(rules, "/additionalFields/values")?
        .get(index)
        .ok_or("the original raw rule value")?;
    proof::present(
        original,
        "value",
        &json!({
            "type":"number","value":{"type":"f64","rawBits":format!("{:016x}", fixture::RAW_BITS),"finite":null}
        }),
        Some(encrypted),
    )?;
    Ok(())
}
