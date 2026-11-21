// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Filter;
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use super::fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const AMMO: &str = "/definitions/ammoDefinitions/rows";
const CONTAINERS: &str = "/configurations/ammoContainers/rows";
const BINDINGS: &str = "/declarations/ammoBindings/rows";
const MAGAZINE: &str = "/projections/weaponMagazineResolutions/rows";

#[test]
fn weapon_filter_keeps_the_own_or_one_hop_magazine_component_and_original_ammo() -> TestResult {
    let source = fixture::database(0.0);
    for (item, owner, selection, outcome, capacity) in [
        (
            "fixture_direct",
            fixture::DIRECT,
            "selected",
            "ownContainer",
            200,
        ),
        (
            "fixture_personal",
            fixture::PRIMARY_MAGAZINE,
            "dependency",
            "linkedContainer",
            30,
        ),
    ] {
        let catalog = filtered(&source, item)?;
        let [container] = proof::rows(&catalog, CONTAINERS)? else {
            return Err("exactly the one demanded container".into());
        };
        assert_eq!(
            proof::at(container, "/selection")?,
            selection,
            "container admission"
        );
        assert_eq!(
            proof::at(container, "/ownerGuid/value")?,
            &json!(common::guid(owner)),
            "physical magazine or item owns the container"
        );
        assert_eq!(
            proof::at(container, "/maxAmmoCount/value")?,
            &json!(capacity),
            "original capacity"
        );
        assert_eq!(
            proof::at(container, "/maxAmmoCount/containsEncryptedSource")?,
            &json!(true),
            "exact capacity encryption origin"
        );
        let [resolution] = proof::rows(&catalog, MAGAZINE)? else {
            return Err("one original primary route".into());
        };
        assert_eq!(
            proof::at(resolution, "/outcome/value/type")?,
            outcome,
            "original route"
        );
        assert_route(container, resolution, &catalog)?;
        assert_eq!(
            proof::rows(&catalog, "/items")?.len(),
            1,
            "a linked magazine dependency does not admit another Item"
        );
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn assert_route(container: &Value, resolution: &Value, catalog: &Value) -> TestResult {
    let [ammo] = proof::rows(catalog, AMMO)? else {
        return Err("only the consulted primary ammunition is materialized".into());
    };
    assert_eq!(
        proof::at(ammo, "/selection")?,
        "dependency",
        "the weapon Filter excludes the Ammo path"
    );
    assert_eq!(
        proof::at(ammo, "/guid/value")?,
        &json!(common::guid(fixture::BULLET)),
        "original shared Ammo identity"
    );
    assert_eq!(
        proof::at(ammo, "/speed/value/finite")?,
        &json!(12.5),
        "raw speed"
    );
    assert_eq!(
        proof::at(ammo, "/speed/containsEncryptedSource")?,
        &json!(true),
        "exact speed encryption origin"
    );
    let binding = proof::rows(catalog, BINDINGS)?
        .iter()
        .find(|binding| binding.get("type").and_then(Value::as_str) == Some("containerSlots"))
        .ok_or("physical container binding")?;
    for key in ["ownerRecordIndex", "componentOrdinal"] {
        assert_eq!(
            proof::at(binding, &format!("/value/{key}/value"))?,
            proof::at(container, &format!("/{key}/value"))?,
            "same physical container key"
        );
    }
    assert_eq!(
        proof::at(binding, "/value/primaryAmmo/value/targetRecordIndex/value")?,
        proof::at(ammo, "/recordIndex/value")?,
        "original primary Reference closes on the Ammo definition"
    );
    assert_eq!(
        proof::at(resolution, "/outcome/value/containerOrdinal/value")?,
        proof::at(container, "/componentOrdinal/value")?,
        "original route reaches that slot"
    );
    Ok(())
}

#[test]
fn a_linked_magazine_without_an_ammo_component_does_not_fabricate_endpoints() -> TestResult {
    let source = fixture::empty_primary_magazine_database();
    let catalog = filtered(&source, "fixture_personal")?;
    assert!(
        proof::rows(&catalog, AMMO)?.is_empty(),
        "no selected or demanded ammunition"
    );
    assert!(
        proof::rows(&catalog, CONTAINERS)?.is_empty(),
        "readable target has no canonical container"
    );
    let [resolution] = proof::rows(&catalog, MAGAZINE)? else {
        return Err("one admitted original route".into());
    };
    assert_eq!(
        proof::at(resolution, "/outcome/value/type")?,
        "linkedWithoutContainer",
        "authoritative original target remains distinguishable"
    );
    proof::closure(&catalog)
}

#[test]
fn weapons_sharing_a_filtered_magazine_keep_each_route_and_one_physical_endpoint() -> TestResult {
    let catalog = filtered(&fixture::shared_magazine_database(), "fixture_personal")?;
    assert_eq!(
        proof::rows(&catalog, MAGAZINE)?.len(),
        2,
        "both weapons retain their actual route"
    );
    assert_eq!(
        proof::rows(&catalog, CONTAINERS)?.len(),
        1,
        "the shared physical container appears once"
    );
    assert_eq!(
        proof::rows(&catalog, AMMO)?.len(),
        1,
        "the shared Ammo definition appears once"
    );
    proof::closure(&catalog)
}

fn filtered(source: &DataForgeBuilder, item: &str) -> TestResult<Value> {
    let (_folder, archive) = common::open(source, DcbOrigin::Encrypted)?;
    let filter = Filter::new(item);
    let options = CatalogOptions {
        filter: Some(&filter),
        ..common::options()
    };
    let built = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}
