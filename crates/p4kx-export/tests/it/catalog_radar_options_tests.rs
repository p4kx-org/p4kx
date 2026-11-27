// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, Filter, build_catalog};
use serde_json::{Value, json};

use super::fixture::{self, Layout};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const ROWS: &str = "/configurations/radarOptions/rows";
const TYPED: &str = "/params/value/radarOptionParams/value";
const PING: &str = "allowFPSPingWave";

#[test]
fn radar_uses_its_own_sourced_options_per_physical_mode() -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&fixture::database(Layout::Complete), origin)?;
        let catalog: Value = serde_json::from_slice(
            build_catalog(&archive, common::options().with_full_provenance())?.json(),
        )?;
        schema
            .validate(&catalog)
            .map_err(|error| format!("catalog schema: {error}"))?;
        let [first, second, alternative, vacant] = proof::rows(&catalog, ROWS)? else {
            return Err("one physical row per GameMode".into());
        };
        check_standard(&catalog, first, 0, true, encrypted)?;
        check_standard(&catalog, second, 1, false, encrypted)?;
        proof::cell(
            proof::at(alternative, "/params/value")?,
            "radarOptionParams",
            "absent",
        )?;
        proof::cell(vacant, "params", "null")?;
        assert_eq!(
            proof::at(first, "/params/sourceUsesRefs")?,
            proof::at(
                proof::rows(&catalog, super::ROWS)?
                    .first()
                    .ok_or("resource network row")?,
                "/params/sourceUsesRefs",
            )?,
            "the two independent outputs retain the same original params admission"
        );
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn historical_radar_field_is_absent_and_parent_filter_still_applies() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(Layout::HistoricalRadar),
        DcbOrigin::Plain,
    )?;
    let filter = Filter::new("*FixtureSecond");
    let options = CatalogOptions {
        filter: Some(&filter),
        ..common::options()
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    let [selected] = proof::rows(&catalog, ROWS)? else {
        return Err("exactly the filtered physical GameMode".into());
    };
    assert_eq!(
        proof::at(selected, "/source/recordIndex/value")?,
        &json!(1),
        "the filter keeps the second physical owner"
    );
    let values = proof::at(selected, TYPED)?;
    proof::cell(values, PING, "absent")?;
    proof::present(values, "useFPSScanMode", &json!(false), Some(false))?;
    proof::closure(&catalog)
}

#[test]
fn malformed_radar_declarations_affect_health_and_retain_original_data() -> TestResult {
    for (layout, state) in [
        (Layout::WrongRadarType, "unsupported"),
        (Layout::WrongRadarStorage, "invalid"),
        (Layout::WrongPingStorage, "invalid"),
    ] {
        let (_folder, archive) = common::open(&fixture::database(layout), DcbOrigin::Encrypted)?;
        assert!(
            matches!(
                build_catalog(&archive, common::options().with_full_provenance()),
                Err(CatalogError::IncompleteExport(_))
            ),
            "normal publication rejects malformed radar storage"
        );
        let options = CatalogOptions {
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..common::options()
        };
        let catalog: Value = serde_json::from_slice(
            build_catalog(&archive, options.with_full_provenance())?.json(),
        )?;
        let first = proof::rows(&catalog, ROWS)?.first().ok_or("radar row")?;
        if matches!(layout, Layout::WrongPingStorage) {
            let radar = proof::at(first, TYPED)?;
            proof::cell(radar, PING, state)?;
            let raw = proof::rows(radar, "/additionalFields/roots")?;
            assert!(
                raw.iter()
                    .any(|row| row.pointer("/name/value") == Some(&json!(PING))),
                "the original malformed field remains inspectable"
            );
        } else {
            let params = proof::at(first, "/params/value")?;
            proof::cell(params, "radarOptionParams", state)?;
            let raw = proof::cell(params, "rawRadarOptionParams", "present")?;
            assert!(
                !proof::rows(raw, "/value/values")?.is_empty(),
                "malformed options retain their original payload"
            );
        }
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn check_standard(
    catalog: &Value,
    row: &Value,
    owner: u32,
    enabled: bool,
    encrypted: bool,
) -> TestResult {
    let params = proof::at(row, "/params/value")?;
    assert_eq!(
        proof::at(row, "/source/recordIndex/value")?,
        &json!(owner),
        "radar values belong to the original physical GameMode"
    );
    proof::present(
        params,
        "sourceType",
        &json!("SStandardParams"),
        Some(encrypted),
    )?;
    let radar = proof::at(row, TYPED)?;
    proof::present(
        radar,
        "sourceType",
        &json!("RadarOptionParams"),
        Some(encrypted),
    )?;
    let mut locations = std::collections::BTreeSet::new();
    for (field, value) in [
        ("useFPSTagging", !enabled),
        ("useFPSScanMode", enabled),
        ("enableScanMode", true),
        ("useExperimentalFeatures", false),
        ("allowDeltaSignature", true),
        ("allowEmissionsHUD", false),
        (PING, enabled),
    ] {
        let cell = proof::present(radar, field, &json!(value), Some(encrypted))?;
        let ordinal = value_ordinal(catalog, cell, owner)?;
        assert!(
            locations.insert(ordinal),
            "each Boolean owns a distinct physical value"
        );
    }
    let [extra] = proof::rows(radar, "/additionalFields/roots")? else {
        return Err("the original future field, without duplicate typed data".into());
    };
    proof::present(
        extra,
        "name",
        &json!("fixtureRawRadarOption"),
        Some(encrypted),
    )?;
    Ok(())
}

fn value_ordinal<'a>(catalog: &'a Value, cell: &Value, owner: u32) -> TestResult<&'a str> {
    let uses = proof::uses(catalog, cell)?;
    let mut payload = uses
        .iter()
        .filter(|source| source.get("role").and_then(Value::as_str) == Some("payload"));
    let body = payload.next().copied().ok_or("original Boolean payload")?;
    assert!(payload.next().is_none(), "one Boolean payload per field");
    assert_eq!(
        proof::at(body, "/location/namespace")?,
        &json!("dataforgeValue"),
        "the Boolean is a DataForge body contribution"
    );
    assert_eq!(
        proof::at(body, "/location/indices/0")?,
        &json!(owner.to_string()),
        "the source coordinate belongs to the original mode"
    );
    assert_eq!(
        proof::at(body, "/location/field")?,
        &json!("$value"),
        "the source location references the stored value"
    );
    proof::at(body, "/location/indices/1")?
        .as_str()
        .ok_or_else(|| "Boolean physical ordinal".into())
}
