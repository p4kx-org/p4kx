// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Contracts and sub contracts in the Basetool catalog, through the public API.

use crate::contracts_fixture;

#[path = "catalog_location_vocabulary.rs"]
mod location_vocabulary;

#[path = "catalog_mission_boards.rs"]
mod mission_boards;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    BuiltCatalog, CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog,
};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

#[path = "catalog_contract_success_rewards.rs"]
mod success_rewards;
#[path = "../contract_v3/mod.rs"]
pub(crate) mod v3;

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(crate) fn build(filter: Option<&Filter>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        contracts_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

pub(crate) fn entry<'a>(
    catalog: &'a Value,
    contract: &str,
    sub: Option<&str>,
) -> TestResult<&'a Value> {
    let mut found = None;
    for row in v3::rows(catalog)? {
        if v3::identity(row)? == (contract, sub) && found.replace(row).is_some() {
            return Err(format!("duplicate contract {contract} {sub:?}").into());
        }
    }
    found.ok_or_else(|| format!("missing contract {contract} {sub:?}").into())
}

pub(crate) fn text<'a>(catalog: &'a Value, row: &Value, field: &str) -> TestResult<v3::Text<'a>> {
    v3::Projections::read(catalog)?.text(row, field)
}

pub(crate) fn entry_by_level(catalog: &Value, sub: bool) -> TestResult<&Value> {
    for row in v3::rows(catalog)? {
        if v3::identity(row)?.1.is_some() == sub {
            return Ok(row);
        }
    }
    Err("missing contract entry for the requested level".into())
}

pub(crate) const A: &str = "20000000-0000-4000-8000-000000000001";
pub(crate) const B: &str = "20000000-0000-4000-8000-000000000002";
pub(crate) const C: &str = "20000000-0000-4000-8000-000000000003";
const D: &str = "20000000-0000-4000-8000-000000000005";
pub(crate) const E: &str = "20000000-0000-4000-8000-000000000006";
const G: &str = "20000000-0000-4000-8000-000000000008";
const H: &str = "20000000-0000-4000-8000-000000000009";
const I: &str = "20000000-0000-4000-8000-00000000000a";
pub(crate) const SUB_ONE: &str = "30000000-0000-4000-8000-000000000001";
const SUB_TWO: &str = "30000000-0000-4000-8000-000000000002";

#[test]
fn a_contract_and_each_sub_contract_is_an_entry_ordered_by_guid() -> TestResult {
    let (built, catalog) = build(None)?;
    let keys = v3::rows(&catalog)?
        .iter()
        .map(v3::identity)
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(
        keys,
        [
            (A, None),
            (A, Some(SUB_ONE)),
            (A, Some(SUB_TWO)),
            (B, None),
            (C, None),
            (D, None),
            (E, None),
            (G, None),
            (H, None),
            (I, None)
        ],
        "the contract, then its sub contracts; the leftover and the contract without an id are out"
    );
    let kind = built.summary().kind(Kind::Contracts);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((10, 7, true)),
        "counts: two duplicates and a contract in a leftover Record are skipped as well"
    );
    Ok(())
}

#[test]
fn a_pair_a_later_generator_record_repeats_is_skipped_and_the_first_record_wins() -> TestResult {
    let (_, catalog) = build(None)?;
    let projections = v3::Projections::read(&catalog)?;
    for sub in [None, Some(SUB_ONE)] {
        assert_eq!(
            projections.header(entry(&catalog, A, sub)?, "guid")?,
            "10000000-0000-4000-8000-000000000001",
            "{sub:?}"
        );
    }
    assert_eq!(entry(&catalog, A, None)?["debugName"], "Deliver_A");
    Ok(())
}

#[test]
fn a_contract_in_a_leftover_record_is_not_listed() -> TestResult {
    let (_, catalog) = build(None)?;
    assert!(entry(&catalog, "20000000-0000-4000-8000-000000000007", None).is_err());
    Ok(())
}

#[test]
fn a_template_reference_that_resolves_to_nothing_leaves_the_template_data_out() -> TestResult {
    let (_, catalog) = build(None)?;
    let dangling = entry(&catalog, G, None)?;
    assert_eq!(
        dangling["templateGuid"],
        Value::Null,
        "a reference to no Record is not a template"
    );
    assert_eq!(
        text(&catalog, dangling, "title")?.en,
        Some("Handler Title"),
        "the override still applies"
    );
    assert_eq!(dangling["missionTypeGuid"], Value::Null);
    assert_eq!(dangling["missionType"]["en"], Value::Null);
    Ok(())
}

#[test]
fn a_template_outside_the_template_folder_is_found_by_its_type() -> TestResult {
    let (_, catalog) = build(None)?;
    let stray = entry(&catalog, H, None)?;
    assert_eq!(
        stray["templateGuid"],
        "40000000-0000-4000-8000-000000000003"
    );
    assert_eq!(text(&catalog, stray, "title")?.en, Some("Plain Title"));
    assert_eq!(stray["missionType"]["en"], "Delivery");
    Ok(())
}

#[test]
fn an_organisation_token_without_a_variant_takes_the_only_string_variant_else_none() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        text(&catalog, entry(&catalog, I, None)?, "title")?.en,
        Some("By [SEVERAL] and Solo Corp"),
        "an organisation with several variants states no choice"
    );
    Ok(())
}

#[test]
fn a_replacement_that_holds_a_token_is_expanded_and_the_contractor_is_the_nearest_string()
-> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        text(&catalog, entry(&catalog, D, None)?, "title")?.en,
        Some("Pre Wrapped [LOCATION]"),
        "a token inside the text of a replacement is resolved too"
    );
    assert_eq!(
        entry(&catalog, A, Some(SUB_ONE))?["contractor"],
        json!({"key": "@contractor_acme", "en": "Acme Agency", "de": null}),
        "the handler's Contractor parameter"
    );
    assert_eq!(
        entry(&catalog, C, None)?["contractor"],
        json!({"key": null, "en": null, "de": null}),
        "a handler without one"
    );
    Ok(())
}

#[test]
fn a_contract_without_a_template_takes_title_type_and_place_from_its_broker_entry() -> TestResult {
    let (_, catalog) = build(None)?;
    let brokered = entry(&catalog, E, None)?;
    assert_eq!(brokered["templateGuid"], Value::Null);
    assert_eq!(text(&catalog, brokered, "title")?.en, Some("Broker Haul"));
    assert_eq!(brokered["missionType"]["en"], "Delivery");
    assert_eq!(places(brokered), [("Planet", "Hurston")]);
    Ok(())
}

#[test]
fn the_faction_of_the_handler_is_exported_as_stored() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        entry(&catalog, A, None)?["faction"],
        json!({
            "guid": "60000000-0000-4000-8000-000000000001",
            "name": "acme",
            "displayName": {"key": "@faction_acme", "en": "Acme Emporium", "de": null},
        })
    );
    assert_eq!(entry(&catalog, C, None)?["faction"], Value::Null);
    Ok(())
}

fn places(entry: &Value) -> Vec<(&str, &str)> {
    entry["pickupPlaces"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|place| {
            (
                place["objectType"].as_str().unwrap_or_default(),
                place
                    .get("place")
                    .and_then(|name| name.get("en"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
        })
        .collect()
}

#[test]
fn the_pickup_places_put_the_system_and_planets_first_and_keep_every_non_star() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        places(entry(&catalog, A, None)?),
        [
            ("SolarSystem", "Stanton"),
            ("Planet", "Hurston"),
            ("Station", "Everus"),
        ],
        "the star is skipped, the system comes first, then the planet, then the rest"
    );
    assert_eq!(
        places(entry(&catalog, A, Some(SUB_ONE))?),
        [("Planet", "Hurston")],
        "a sub contract's own location replaces the contract's"
    );
    assert_eq!(
        entry(&catalog, A, Some(SUB_TWO))?["pickupPlaces"],
        entry(&catalog, A, None)?["pickupPlaces"],
        "a sub contract without places takes the contract's"
    );
    assert_eq!(
        places(entry(&catalog, B, None)?).len(),
        7,
        "every eligible original place"
    );
    assert_eq!(places(entry(&catalog, C, None)?), [], "none named");
    Ok(())
}

#[test]
fn the_title_resolves_tokens_and_writes_placeholders_for_what_the_game_picks() -> TestResult {
    let (_, catalog) = build(None)?;
    let projections = v3::Projections::read(&catalog)?;
    let parent = projections.text(entry(&catalog, A, None)?, "title")?;
    assert_eq!(
        parent,
        v3::Text {
            key: Some("@title_a"),
            en: Some("Deliver Scrap to [LOCATION] for Acme Corp [MISSING]"),
            de: Some("Liefere [CARGO] nach [LOCATION] fuer Acme GmbH"),
        },
        "the contract's property wins over the template's; the variant picks the tagged string; \
         an unknown token and a key without a text become [TOKEN]; blanks are trimmed"
    );
    assert_eq!(
        projections.text(entry(&catalog, A, Some(SUB_ONE))?, "title")?,
        v3::Text {
            key: Some("@title_sub"),
            en: Some("Stop [TARGET] in [SHIP]"),
            de: None
        },
        "the sub contract's own title"
    );
    assert_eq!(
        projections.text(entry(&catalog, A, Some(SUB_TWO))?, "title")?,
        parent,
        "a sub contract without a title takes the contract's"
    );
    Ok(())
}

#[test]
fn the_title_falls_back_to_the_handler_and_then_the_template() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        text(&catalog, entry(&catalog, B, None)?, "title")?,
        v3::Text {
            key: Some("@title_template"),
            en: Some("Plain Delivery"),
            de: None
        },
        "the template's first display string, trimmed"
    );
    assert_eq!(
        text(&catalog, entry(&catalog, C, None)?, "title")?.en,
        Some("Handler Title"),
        "the handler's title wins over the template's"
    );
    Ok(())
}

#[test]
fn the_mission_type_is_the_nearest_override_else_the_template_type() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        entry(&catalog, A, None)?["missionType"],
        json!({"key": "@type_delivery", "en": "Delivery", "de": "Lieferung"}),
        "the template's type"
    );
    assert_eq!(
        entry(&catalog, A, Some(SUB_ONE))?["missionType"]["en"],
        "Escort",
        "the sub contract's override"
    );
    assert_eq!(
        entry(&catalog, C, None)?["missionType"]["en"],
        "Bounty",
        "the handler's override"
    );
    assert_eq!(
        entry(&catalog, C, None)?["missionTypeGuid"],
        "50000000-0000-4000-8000-000000000002"
    );
    Ok(())
}

#[test]
fn an_entry_carries_its_record_template_handler_and_flags() -> TestResult {
    let (_, catalog) = build(None)?;
    let first = entry(&catalog, A, None)?;
    let projections = v3::Projections::read(&catalog)?;
    assert_eq!(
        projections.header(first, "guid")?,
        "10000000-0000-4000-8000-000000000001"
    );
    assert_eq!(
        projections.header(first, "recordPath")?,
        "libs/foundry/records/contracts/contractgenerator/delivery"
    );
    assert_eq!(first["debugName"], "Deliver_A");
    assert_eq!(first["handler"], "Delivery_Career");
    assert_eq!(
        first["templateGuid"],
        "40000000-0000-4000-8000-000000000001"
    );
    Ok(())
}

#[test]
fn a_filter_that_matches_no_generator_leaves_no_contract_and_marks_the_kind_partial() -> TestResult
{
    let filter = Filter::new("no_such_generator");
    let (built, catalog) = build(Some(&filter))?;
    assert!(v3::rows(&catalog)?.is_empty(), "no contract projection");
    assert_eq!(
        built
            .summary()
            .kind(Kind::Contracts)
            .map(|kind| kind.complete),
        Some(false)
    );
    Ok(())
}
