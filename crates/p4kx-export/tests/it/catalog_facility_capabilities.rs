// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::{Archive, DataForgeEntry, StructValue, Value as Original};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_facility_capabilities_fixture.rs"]
mod fixture;

#[test]
fn original_facility_capabilities_join_each_resolved_ship_placement() -> TestResult {
    let (_directory, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    verify_original(&archive)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let [hanger, logout, empty] = proof::rows(&catalog, "/configurations/facilities/rows")? else {
        return Err("the three physical facility components".into());
    };
    check_outfit_hanger(hanger)?;
    check_logout(logout)?;
    proof::present(empty, "logoutInteractions", &json!([]), Some(true))?;
    check_placements(&catalog)?;
    check_personal_weapon_port(&catalog)?;
    jsonschema::validator_for(&p4kx_export::catalog_schema())?
        .validate(&catalog)
        .map_err(|error| format!("schema: {error}"))?;
    proof::closure(&catalog)
}

fn verify_original(archive: &Archive) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let owner = forge.record(0).ok_or("fixture owner Record")?.tree()?;
    let Some(Original::Array(components)) = owner.field("Components") else {
        return Err("fixture Components array".into());
    };
    let [_, _, Original::Struct(hanger), Original::Struct(logout), _] = components.as_ref() else {
        return Err("physical outfit hanger and repeated logout components".into());
    };
    assert_eq!(
        hanger.type_name(),
        fixture::OUTFIT,
        "original outfit hanger component"
    );
    assert_eq!(
        logout.type_name(),
        fixture::LOGOUT,
        "original logout component"
    );
    check_weak_target(hanger, "equipInteraction");
    let Some(Original::Array(interactions)) = logout.field("logoutInteractions") else {
        return Err("original ordered logout interactions".into());
    };
    let [
        Original::Struct(first),
        Original::Null,
        Original::Struct(last),
    ] = interactions.as_ref()
    else {
        return Err("original distinct interaction and null slots".into());
    };
    check_weak_target(first, "logoutInteractionPoint");
    check_weak_target(last, "respawnInteraction");
    Ok(())
}

fn check_weak_target(source: &StructValue<'_>, field: &str) {
    assert!(
        matches!(source.field(field), Some(Original::Reference(target)) if target.location().is_some()),
        "{field} binds to the original owned-instance target"
    );
}

fn check_outfit_hanger(row: &Value) -> TestResult {
    assert_eq!(
        row["kind"], "outfitHanger",
        "declared hanger component kind"
    );
    proof::present(row, "sourceType", &json!(fixture::OUTFIT), Some(true))?;
    proof::present(row, "componentOrdinal", &json!(2), Some(true))?;
    proof::present(row, "destroyPlayerItems", &json!(false), Some(true))?;
    proof::cell(row, "hangInteraction", "null")?;
    proof::cell(row, "animatedOutfitHang", "null")?;
    check_owner_guid(proof::at(row, "/equipInteraction/value")?, "recordGuid")?;
    check_owner_guid(
        proof::at(row, "/disguiseSwapAllInteraction/value")?,
        "recordGuid",
    )?;
    let [first, second] = proof::rows(row, "/itemPortTypeSubtypes/value")? else {
        return Err("both original outfit hanger restrictions".into());
    };
    for (restriction, kind, subtype) in [
        (first, "FixtureTypeB", "FixtureSubtypeA"),
        (second, "FixtureTypeA", "FixtureSubtypeB"),
    ] {
        let restriction = proof::at(restriction, "/value")?;
        proof::present(restriction, "itemType", &json!(kind), Some(true))?;
        proof::present(restriction, "itemSubType", &json!(subtype), Some(true))?;
    }
    let animation = proof::at(row, "/animatedOutfitSwap/value/values/0/value/value")?;
    proof::present(
        animation,
        "sourceType",
        &json!(fixture::ANIMATION),
        Some(true),
    )?;
    let duration = proof::at(row, "/animatedOutfitSwap/value/values/1/value/value/value")?;
    assert_eq!(
        duration["rawBits"], "80000000",
        "concrete animation data retains exact Single bits"
    );
    Ok(())
}

fn check_logout(row: &Value) -> TestResult {
    assert_eq!(row["kind"], "logout", "declared logout component kind");
    proof::present(row, "sourceType", &json!(fixture::LOGOUT), Some(true))?;
    proof::present(row, "componentOrdinal", &json!(3), Some(true))?;
    let [first, gap, last] = proof::rows(row, "/logoutInteractions/value")? else {
        return Err("first and last source slots with the original gap".into());
    };
    assert_eq!(
        gap["state"], "null",
        "the null pointer keeps its original ordinal"
    );
    assert_eq!(
        gap["containsEncryptedSource"], true,
        "null remains physically sourced"
    );
    let first = proof::at(first, "/value")?;
    let last = proof::at(last, "/value")?;
    proof::cell(first, "respawnInteraction", "null")?;
    proof::cell(last, "logoutInteraction", "null")?;
    for (row, field) in [
        (first, "logoutInteraction"),
        (first, "logoutInteractionPoint"),
        (last, "respawnInteraction"),
        (last, "respawnInteractionPoint"),
    ] {
        let link = proof::cell(row, field, "present")?;
        check_owner_guid(proof::at(link, "/value")?, "recordGuid")?;
    }
    Ok(())
}

fn check_placements(catalog: &Value) -> TestResult {
    let rows = proof::rows(catalog, "/placements/facilityInstances/rows")?;
    assert_eq!(
        rows.len(),
        2,
        "the two attachments keep separate placements"
    );
    for (ordinal, row) in rows.iter().enumerate() {
        assert_eq!(
            proof::at(row, "/source/position/value/portOrdinals")?,
            &json!([ordinal]),
            "placement follows the exact original Loadout port"
        );
        let linked = proof::rows(row, "/configurations")?;
        assert_eq!(
            linked.len(),
            3,
            "all original capability components join to each placement"
        );
        for (index, link) in linked.iter().enumerate() {
            let owner = proof::at(link, "/owner")?;
            check_owner_guid(owner, "ownerGuid")?;
            proof::present(owner, "componentOrdinal", &json!(index + 2), Some(true))?;
        }
    }
    Ok(())
}

fn check_owner_guid(row: &Value, field: &str) -> TestResult {
    proof::present(row, field, &json!(common::guid(fixture::OWNER)), Some(true))?;
    Ok(())
}

fn check_personal_weapon_port(catalog: &Value) -> TestResult {
    let row = proof::at(catalog, "/configurations/itemPorts/rows/0")?;
    check_owner_guid(row, "ownerGuid")?;
    let port = proof::at(row, "/ports/value/0/value")?;
    proof::present(port, "portOrdinal", &json!(0), Some(true))?;
    let restrictions = proof::at(port, "/restrictions")?;
    proof::present(
        restrictions,
        "minSize",
        &json!({"type":"uint32","value":1}),
        Some(true),
    )?;
    proof::present(
        restrictions,
        "maxSize",
        &json!({"type":"uint32","value":3}),
        Some(true),
    )?;
    proof::present(
        restrictions,
        "requiredPortTags",
        &json!("FixturePersonal"),
        Some(true),
    )?;
    let accepted = proof::at(restrictions, "/types/value/0/value")?;
    proof::present(accepted, "type", &json!("WeaponPersonal"), Some(true))?;
    assert_eq!(
        proof::rows(accepted, "/subTypes/value/values/0/value/value/elements")?.len(),
        2,
        "the canonical generic port table retains both stored subtype restrictions"
    );
    Ok(())
}
