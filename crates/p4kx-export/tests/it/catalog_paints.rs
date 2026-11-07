// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Paints and the ships they fit in the Basetool catalog, through the public API.

use crate::paints_fixture;

use std::collections::BTreeSet;
use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    BuiltCatalog, CatalogOptions, CraftingWarning, Filter, Kind, PaintSummary, build_catalog,
};
use p4kx_extraction::Archive;
use paints_fixture::{
    PAINT_ALPHA_RED, PAINT_ALPHA_VARIANT, PAINT_BETA, PAINT_DELTA_MIL, PAINT_DELTA_PORT,
    PAINT_ETA_TREE, PAINT_GAMMA, PAINT_ORPHAN, PAINT_UPPER, PAINT_ZETA, PAINT_ZETA_CASE,
    PAINT_ZETA_REQ, PAINT_ZETA_TWO, SHIP_ALPHA, SHIP_BETA, SHIP_DELTA, SHIP_EPSILON, SHIP_ETA,
    SHIP_GAMMA, SHIP_MISSING, SHIP_ZETA, write_archive,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);

fn build(filter: Option<&Filter>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT);
    options.publication_mode = p4kx_export::CatalogPublicationMode::DiagnosticPartial;
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn guid(value: u128) -> String {
    let text = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    )
}

fn ships_of(catalog: &Value, paint: u128) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let wanted = guid(paint);
    let entry = catalog["paints"]
        .as_array()
        .ok_or("paints")?
        .iter()
        .find(|entry| entry["itemGuid"] == wanted.as_str())
        .ok_or("the paint")?;
    Ok(entry["shipGuids"]
        .as_array()
        .ok_or("shipGuids")?
        .iter()
        .filter_map(|ship| ship.as_str().map(str::to_owned))
        .collect())
}

fn set(ships: &[u128]) -> BTreeSet<String> {
    ships.iter().copied().map(guid).collect()
}

#[test]
fn a_paint_fits_the_ships_whose_paint_port_requires_its_tags() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        ships_of(&catalog, PAINT_ALPHA_RED)?,
        set(&[SHIP_ALPHA]),
        "a required tag among the paint's tags; the Modification Beta moves the other ship"
    );
    assert_eq!(
        ships_of(&catalog, PAINT_BETA)?,
        set(&[SHIP_BETA]),
        "the Elem of the Modification overrides the required tags of the port by id"
    );
    assert_eq!(
        ships_of(&catalog, PAINT_GAMMA)?,
        set(&[SHIP_GAMMA]),
        "a paint port in the Record of the ship"
    );
    Ok(())
}

#[test]
fn a_tag_below_the_required_tag_in_the_tag_tree_and_a_tag_in_other_case_match() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        ships_of(&catalog, PAINT_ALPHA_VARIANT)?,
        set(&[SHIP_ALPHA]),
        "Paint_Alpha_Variant lies below Paint_Alpha"
    );
    assert_eq!(
        ships_of(&catalog, PAINT_UPPER)?,
        set(&[SHIP_ALPHA]),
        "tag names compare ignoring case"
    );
    Ok(())
}

#[test]
fn the_required_tags_of_a_paint_must_be_offered_by_the_port_or_the_ship() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        ships_of(&catalog, PAINT_DELTA_MIL)?,
        set(&[SHIP_DELTA]),
        "the Mil tag is one of the Record tags of the Delta alone"
    );
    assert_eq!(
        ships_of(&catalog, PAINT_DELTA_PORT)?,
        set(&[SHIP_DELTA, SHIP_EPSILON]),
        "a required tag the port lists is offered by the port"
    );
    Ok(())
}

#[test]
fn a_required_tag_may_be_a_port_tag_a_required_port_tag_or_in_other_case() -> TestResult {
    let (_, catalog) = build(None)?;
    for (paint, why) in [
        (
            PAINT_ZETA,
            "the port tag differs from the required port tag",
        ),
        (PAINT_ZETA_REQ, "the required port tag of the port"),
        (PAINT_ZETA_CASE, "tag names compare ignoring case"),
    ] {
        assert_eq!(ships_of(&catalog, paint)?, set(&[SHIP_ZETA]), "{why}");
    }
    Ok(())
}

#[test]
fn every_required_tag_of_a_paint_must_be_offered() -> TestResult {
    let (_, catalog) = build(None)?;
    assert!(
        ships_of(&catalog, PAINT_ZETA_TWO)?.is_empty(),
        "the port offers one of the two tags, no ship tag the other"
    );
    Ok(())
}

#[test]
fn an_offered_tag_below_the_required_tag_in_the_tag_tree_counts() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        ships_of(&catalog, PAINT_ETA_TREE)?,
        set(&[SHIP_ETA]),
        "the Record tag Paint_Alpha_Variant lies below the required Paint_Alpha"
    );
    Ok(())
}

#[test]
fn a_derived_kind_has_no_record_rule() {
    for kind in Kind::ALL {
        assert_eq!(kind.rule().is_none(), kind.is_derived(), "{kind}");
    }
    assert!(Kind::Paints.rule().is_none(), "paints");
}

#[test]
fn a_port_for_other_types_assigns_no_paint() -> TestResult {
    let (_, catalog) = build(None)?;
    assert!(
        ships_of(&catalog, PAINT_ORPHAN)?.is_empty(),
        "the port that requires Paint_Orphan takes weapons"
    );
    Ok(())
}

#[test]
fn the_summary_counts_the_paints_with_and_without_a_ship() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        built.summary().paints(),
        PaintSummary {
            paints: 13,
            assigned: 11,
            unassigned: 2,
            ships: 7,
            unreadable_vehicles: 1,
        },
        "the corrupt vehicle file of one ship is counted, the file the Archive lacks is not"
    );
    let [CraftingWarning::UnreadableVehicleFiles { count: 1, details }] =
        built.summary().warnings()
    else {
        return Err(format!("one warning: {:?}", built.summary().warnings()).into());
    };
    let detail = details.first().ok_or("the detail")?;
    assert_eq!(details.len(), 1, "{details:?}");
    assert!(
        detail.to_ascii_lowercase().contains("acas_broken.xml"),
        "the warning names the file: {details:?}"
    );
    let missing = guid(SHIP_MISSING);
    let loadout = catalog["loadouts"]
        .as_array()
        .ok_or("loadouts")?
        .iter()
        .find(|entry| entry["entityGuid"] == missing.as_str())
        .ok_or("the loadout of the ship without a vehicle file")?;
    assert!(
        loadout["issues"]
            .as_array()
            .ok_or("issues")?
            .iter()
            .any(|issue| issue == "vehicle_implementation"),
        "a missing implementation stays reported as a loadout issue: {loadout}"
    );
    let kind = built.summary().kind(Kind::Paints).ok_or("the kind")?;
    assert_eq!(
        (kind.count, kind.skipped, kind.complete),
        (13, 1, true),
        "the kind summary counts the leftover paint as skipped"
    );
    let items = built.summary().kind(Kind::Items).ok_or("the items")?;
    assert_eq!(
        items.skipped, 2,
        "the leftover paint is a skipped item too, next to the leftover power plant"
    );
    assert_eq!(
        built.summary().skipped(),
        2,
        "the leftover paint is counted once in the total"
    );
    assert_eq!(
        catalog["completeness"]["paints"]["status"], "complete",
        "completeness"
    );
    Ok(())
}

#[test]
fn every_ship_of_an_assignment_is_a_catalog_ship() -> TestResult {
    let (_, catalog) = build(None)?;
    let ships: BTreeSet<&str> = catalog["ships"]
        .as_array()
        .ok_or("ships")?
        .iter()
        .filter_map(|ship| ship["guid"].as_str())
        .collect();
    let items: BTreeSet<&str> = catalog["items"]
        .as_array()
        .ok_or("items")?
        .iter()
        .filter_map(|item| item["guid"].as_str())
        .collect();
    for paint in catalog["paints"].as_array().ok_or("paints")? {
        assert!(
            items.contains(paint["itemGuid"].as_str().ok_or("itemGuid")?),
            "the item of the paint is in items"
        );
        let listed: Vec<&str> = paint["shipGuids"]
            .as_array()
            .ok_or("shipGuids")?
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(listed.is_sorted(), "ordered by GUID");
        assert!(listed.iter().all(|ship| ships.contains(ship)), "resolves");
    }
    Ok(())
}

#[test]
fn paints_are_ordered_by_item_guid_and_reference_the_item_only() -> TestResult {
    let (_, catalog) = build(None)?;
    let paints = catalog["paints"].as_array().ok_or("paints")?;
    let guids: Vec<&str> = paints
        .iter()
        .filter_map(|paint| paint["itemGuid"].as_str())
        .collect();
    assert!(guids.is_sorted(), "ordered by the GUID of the item");
    let mut fields: Vec<&str> = paints
        .first()
        .and_then(Value::as_object)
        .ok_or("a paint")?
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        ["itemGuid", "shipGuids"],
        "name and record stay on the item; no description, no media"
    );
    Ok(())
}

#[test]
fn a_development_leftover_is_neither_an_item_nor_a_paint() -> TestResult {
    let (_, catalog) = build(None)?;
    let leftover = guid(paints_fixture::PAINT_TEST);
    for kind in ["items", "paints"] {
        let own = if kind == "items" { "guid" } else { "itemGuid" };
        assert!(
            catalog[kind]
                .as_array()
                .ok_or("kind")?
                .iter()
                .all(|entry| entry[own] != leftover.as_str()),
            "{kind} leaves out paint_test"
        );
    }
    Ok(())
}

#[test]
fn a_filter_makes_the_paints_partial_and_assigns_only_the_selected_ships() -> TestResult {
    let filter = Filter::new("paint_beta");
    let (built, catalog) = build(Some(&filter))?;
    assert_eq!(
        catalog["completeness"]["paints"]["status"], "partial",
        "partial"
    );
    assert_eq!(
        catalog["paints"].as_array().ok_or("paints")?.len(),
        1,
        "only the paint the Filter matches"
    );
    assert!(
        ships_of(&catalog, PAINT_BETA)?.is_empty(),
        "the ships are filtered as well, so none of them is listed"
    );
    assert_eq!(built.summary().paints().unassigned, 1, "counted");
    Ok(())
}
