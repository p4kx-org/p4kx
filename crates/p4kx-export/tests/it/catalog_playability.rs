// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The playability of ships and items in the Basetool catalog, through the public API.

use crate::playability_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, Playability, PlayabilityCounts, build_catalog};
use p4kx_extraction::Archive;
use playability_fixture::{Corrupt, write_archive_with};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);

fn catalog() -> Result<(p4kx_export::BuiltCatalog, Value), Box<dyn Error>> {
    catalog_mode(
        Corrupt::Nothing,
        p4kx_export::CatalogPublicationMode::Normal,
    )
}

fn catalog_of(corrupt: Corrupt) -> Result<(p4kx_export::BuiltCatalog, Value), Box<dyn Error>> {
    catalog_mode(
        corrupt,
        p4kx_export::CatalogPublicationMode::DiagnosticPartial,
    )
}

fn catalog_mode(
    corrupt: Corrupt,
    publication_mode: p4kx_export::CatalogPublicationMode,
) -> Result<(p4kx_export::BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        write_archive_with(dir.path(), corrupt)?,
        Some(p4kx_testkit::test_key()),
    )?;
    let built = build_catalog(
        &archive,
        &CatalogOptions {
            publication_mode,
            ..CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
        },
    )?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn playability<'c>(catalog: &'c Value, kind: &str, class: &str) -> Result<&'c Value, String> {
    catalog[kind]
        .as_array()
        .and_then(|list| list.iter().find(|entry| entry["className"] == class))
        .map(|entry| &entry["playability"])
        .ok_or_else(|| format!("{kind} {class} missing"))
}

fn assert_ships(catalog: &Value, expected: &[(&str, Option<&str>)]) -> TestResult {
    for (class, value) in expected {
        let actual = playability(catalog, "ships", class)?;
        assert_eq!(
            actual,
            &value.map_or(Value::Null, Value::from),
            "ship {class}"
        );
    }
    Ok(())
}

#[test]
fn a_ship_without_ai_evidence_that_the_game_ships_is_a_player_ship() -> TestResult {
    let (_, catalog) = catalog()?;
    assert_ships(
        &catalog,
        &[
            ("Ship_Player", Some("player")),
            ("Ship_Empty_Manifest", Some("player")),
            ("Ship_Ready_Explosive_Hull", Some("player")),
        ],
    )
}

#[test]
fn a_crew_manifest_that_lists_a_crew_or_an_explosive_hull_makes_a_ship_ai() -> TestResult {
    let (_, catalog) = catalog()?;
    assert_ships(
        &catalog,
        &[
            ("Ship_Crewed", Some("ai")),
            ("Ship_Explosive", Some("ai")),
            ("Ship_Hull_Not_Shipped", Some("ai")),
        ],
    )
}

#[test]
fn an_unmanned_tag_makes_a_ship_a_mission_ship_and_an_unshipped_one_special() -> TestResult {
    let (_, catalog) = catalog()?;
    assert_ships(
        &catalog,
        &[
            ("Ship_Salvage", Some("mission")),
            ("Ship_Not_Shipped", Some("special")),
        ],
    )
}

#[test]
fn a_systems_design_record_tag_makes_a_ship_ai_whatever_else_it_carries() -> TestResult {
    let (_, catalog) = catalog()?;
    assert_ships(
        &catalog,
        &[
            ("Ship_Systems_Design", Some("ai")),
            ("Ship_Systems_Design_Not_Shipped", Some("ai")),
            ("Ship_Systems_Design_Salvage", Some("ai")),
            ("Ship_Other_Record_Tag", Some("player")),
            ("Rover_Systems_Design", Some("ai")),
        ],
    )
}

#[test]
fn a_ship_the_rules_cannot_place_has_no_playability() -> TestResult {
    let (_, catalog) = catalog()?;
    assert_ships(&catalog, &[("Ship_No_Inclusion", None)])
}

#[test]
fn the_hull_signals_apply_to_spaceships_only() -> TestResult {
    let (_, catalog) = catalog()?;
    assert_ships(
        &catalog,
        &[
            ("Rover_Player", Some("player")),
            ("Rover_Salvage", Some("mission")),
        ],
    )
}

#[test]
fn an_item_with_a_shop_display_is_a_player_item() -> TestResult {
    let (_, catalog) = catalog()?;
    for class in ["Item_Shop", "Item_Shop_Without_Name"] {
        assert_eq!(playability(&catalog, "items", class)?, "player", "{class}");
    }
    Ok(())
}

#[test]
fn an_item_of_a_mission_folder_without_a_shop_display_is_a_mission_item() -> TestResult {
    let (_, catalog) = catalog()?;
    for class in ["Item_Mission", "Item_Mission_Stash"] {
        assert_eq!(playability(&catalog, "items", class)?, "mission", "{class}");
    }
    Ok(())
}

#[test]
fn an_item_without_a_player_visible_name_or_shop_display_is_special() -> TestResult {
    let (_, catalog) = catalog()?;
    for class in ["Item_Nameless", "Item_Old"] {
        assert_eq!(playability(&catalog, "items", class)?, "special", "{class}");
    }
    Ok(())
}

#[test]
fn a_named_item_without_a_shop_display_has_no_playability() -> TestResult {
    let (_, catalog) = catalog()?;
    assert_eq!(playability(&catalog, "items", "Item_Named")?, &Value::Null);
    Ok(())
}

#[test]
fn every_ship_and_item_carries_the_field() -> TestResult {
    let (_, catalog) = catalog()?;
    for kind in ["ships", "items"] {
        let entries = catalog[kind].as_array().ok_or("kind missing")?;
        assert!(!entries.is_empty(), "{kind} present");
        assert!(
            entries
                .iter()
                .all(|entry| entry.get("playability").is_some()),
            "{kind}: every entry has the field"
        );
    }
    Ok(())
}

#[test]
fn the_summary_counts_every_value_and_the_unclassified() -> TestResult {
    let (built, _) = catalog()?;
    let summary = built.summary().playability();
    let expect = |counts: PlayabilityCounts, values: [usize; 4], unclassified: usize| {
        let actual = Playability::ALL.map(|value| counts.of(value));
        assert_eq!(actual, values, "player, ai, mission, special");
        assert_eq!(counts.unclassified(), unclassified, "unclassified");
    };
    expect(summary.ships, [5, 9, 3, 1], 2);
    expect(summary.items, [3, 0, 2, 2], 1);
    Ok(())
}

#[test]
fn the_rules_apply_in_order() -> TestResult {
    let (_, catalog) = catalog()?;
    assert_ships(
        &catalog,
        &[
            ("Ship_Crewed_Salvage", Some("ai")),
            ("Ship_Not_Shipped_Salvage", Some("mission")),
            ("Rover_Crewed", Some("ai")),
        ],
    )?;
    assert_eq!(
        playability(&catalog, "items", "Item_Mission_Shop")?,
        "player",
        "a shop display wins over the mission folder"
    );
    Ok(())
}

#[test]
fn any_other_inclusion_mode_leaves_a_ship_without_playability() -> TestResult {
    let (_, catalog) = catalog()?;
    assert_ships(&catalog, &[("Ship_Other_Inclusion", None)])
}

#[test]
fn a_ship_linking_to_an_unreadable_crew_manifest_has_no_playability() -> TestResult {
    let (built, catalog) = catalog_of(Corrupt::Manifests)?;
    assert!(!built.summary().is_complete(), "the manifests failed");
    assert_ships(
        &catalog,
        &[
            ("Ship_Crewed", None),
            ("Ship_Empty_Manifest", None),
            ("Rover_Crewed", None),
            ("Ship_Player", Some("player")),
        ],
    )
}

#[test]
fn a_ship_linking_to_an_unreadable_tag_has_no_playability() -> TestResult {
    let (built, catalog) = catalog_of(Corrupt::Tags)?;
    assert!(!built.summary().is_complete(), "the tags failed");
    assert_ships(
        &catalog,
        &[
            ("Ship_Player", None),
            ("Ship_Salvage", None),
            ("Ship_Crewed", None),
            ("Rover_Player", None),
        ],
    )
}
