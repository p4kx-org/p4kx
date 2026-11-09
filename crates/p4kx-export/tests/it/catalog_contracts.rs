// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Contracts and sub contracts in the Basetool catalog, through the public API.

use crate::contracts_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    BuiltCatalog, CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog,
};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

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
) -> Result<&'a Value, String> {
    catalog["contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| {
            entry["guid"] == contract
                && entry["subContractGuid"] == sub.map_or(Value::Null, Value::from)
        })
        .ok_or_else(|| format!("{contract} {sub:?}"))
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
    let keys: Vec<(&str, Option<&str>)> = catalog["contracts"]
        .as_array()
        .ok_or("contracts")?
        .iter()
        .map(|entry| {
            (
                entry["guid"].as_str().unwrap_or_default(),
                entry["subContractGuid"].as_str(),
            )
        })
        .collect();
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
    for sub in [None, Some(SUB_ONE)] {
        assert_eq!(
            entry(&catalog, A, sub)?["recordGuid"],
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
        dangling["title"]["en"], "Handler Title",
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
    assert_eq!(stray["title"]["en"], "Plain Title");
    assert_eq!(stray["missionType"]["en"], "Delivery");
    Ok(())
}

#[test]
fn an_organisation_token_without_a_variant_takes_the_only_string_variant_else_none() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        entry(&catalog, I, None)?["title"]["en"],
        "By [SEVERAL] and Solo Corp",
        "an organisation with several variants states no choice"
    );
    Ok(())
}

#[test]
fn a_replacement_that_holds_a_token_is_expanded_and_the_contractor_is_the_nearest_string()
-> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        entry(&catalog, D, None)?["title"]["en"],
        "Pre Wrapped [LOCATION]",
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
    assert_eq!(brokered["title"]["en"], "Broker Haul");
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
    assert_eq!(
        entry(&catalog, A, None)?["title"],
        json!({
            "key": "@title_a",
            "en": "Deliver Scrap to [LOCATION] for Acme Corp [MISSING]",
            "de": "Liefere [CARGO] nach [LOCATION] fuer Acme GmbH",
        }),
        "the contract's property wins over the template's; the variant picks the tagged string; \
         an unknown token and a key without a text become [TOKEN]; blanks are trimmed"
    );
    assert_eq!(
        entry(&catalog, A, Some(SUB_ONE))?["title"],
        json!({"key": "@title_sub", "en": "Stop [TARGET] in [SHIP]", "de": null}),
        "the sub contract's own title"
    );
    assert_eq!(
        entry(&catalog, A, Some(SUB_TWO))?["title"],
        entry(&catalog, A, None)?["title"],
        "a sub contract without a title takes the contract's"
    );
    Ok(())
}

#[test]
fn the_title_falls_back_to_the_handler_and_then_the_template() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        entry(&catalog, B, None)?["title"],
        json!({"key": "@title_template", "en": "Plain Delivery", "de": null}),
        "the template's first display string, trimmed"
    );
    assert_eq!(
        entry(&catalog, C, None)?["title"]["en"],
        "Handler Title",
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
fn only_the_reputation_a_contract_grants_on_success_is_listed() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        entry(&catalog, A, None)?["reputationGains"],
        json!([
            {
                "factionReputationGuid": "60000000-0000-4000-8000-000000000001",
                "factionReputationName": "acme",
                "scopeGuid": "60000000-0000-4000-8000-000000000002",
                "scopeName": "faction",
                "amount": 200,
            },
            {
                "factionReputationGuid": "60000000-0000-4000-8000-000000000001",
                "factionReputationName": "acme",
                "scopeGuid": "60000000-0000-4000-8000-000000000003",
                "scopeName": "affinity",
                "amount": 50,
            },
        ]),
        "faction and affinity scope; the failure result with -100 is not listed"
    );
    assert_eq!(entry(&catalog, B, None)?["reputationGains"], json!([]));
    Ok(())
}

#[test]
fn a_payout_the_contract_states_is_listed_as_stored_and_only_for_success() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        entry(&catalog, A, None)?["fixedRewards"],
        json!([
            {"currency": "UEC", "amount": 40000, "max": 0, "plusBonuses": false},
            {"currency": null, "amount": 7, "max": null, "plusBonuses": null}
        ]),
        "the failure payout of 1000 is not listed; fields the reward lacks are null"
    );
    assert_eq!(entry(&catalog, B, None)?["fixedRewards"], json!([]));
    Ok(())
}

#[test]
fn an_entry_carries_its_record_template_handler_and_flags() -> TestResult {
    let (_, catalog) = build(None)?;
    let first = entry(&catalog, A, None)?;
    assert_eq!(first["recordGuid"], "10000000-0000-4000-8000-000000000001");
    assert_eq!(
        first["recordPath"],
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
    assert_eq!(catalog["contracts"], json!([]));
    assert_eq!(
        built
            .summary()
            .kind(Kind::Contracts)
            .map(|kind| kind.complete),
        Some(false)
    );
    Ok(())
}
