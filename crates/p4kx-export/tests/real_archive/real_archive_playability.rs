// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The playability of ships and items in the P4KX catalog of a real Archive; runs with
//! `--ignored` when `P4KX_DATA_P4K` names one.

use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, Kind, Playability, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::real_archive_path;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

fn playability<'c>(catalog: &'c Value, kind: &str, class: &str) -> Result<&'c Value, String> {
    let family = Kind::ALL
        .into_iter()
        .find(|family| family.name() == kind)
        .ok_or("known entity family")?;
    let rows =
        crate::catalog_v3_consumer::rows(catalog, family).map_err(|error| error.to_string())?;
    for entry in rows {
        let name = crate::catalog_v3_consumer::field(entry, family, "className")
            .map_err(|error| error.to_string())?;
        if name == class {
            return crate::catalog_v3_consumer::field(entry, family, "playability")
                .map_err(|error| error.to_string());
        }
    }
    Err(format!("{kind} {class} missing"))
}

#[expect(clippy::print_stdout, reason = "the counts are the result of the run")]
fn print_line(line: &str) {
    println!("{line}");
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn known_ships_and_items_of_a_real_archive_have_the_expected_playability() -> TestResult {
    let archive =
        Archive::open_with_key(real_archive_path()?, p4kx_testkit::real_archive_key().ok())?;
    let built = build_catalog(
        &archive,
        CatalogOptions::new(SystemTime::now()).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let summary = built.summary().playability();
    for (kind, counts) in [("items", summary.items), ("ships", summary.ships)] {
        let values = Playability::ALL.map(|value| format!("{} {}", counts.of(value), value.name()));
        print_line(&format!(
            "{kind}: {}, {} unclassified",
            values.join(", "),
            counts.unclassified()
        ));
    }
    for (kind, class, expected) in [
        ("ships", "AEGS_Avenger_Titan", "player"),
        ("ships", "AEGS_Avenger_Titan_PU_AI_CIV", "ai"),
        ("ships", "ANVL_Hornet_F7C_Mk2", "player"),
        ("ships", "ANVL_Hornet_F7C_Mk2_AI_Super", "ai"),
        ("ships", "RSI_Perseus", "player"),
        ("ships", "RSI_Perseus_AI_Super", "ai"),
        ("ships", "DRAK_Corsair_AI_Super", "ai"),
        ("ships", "RSI_Skylark", "player"),
        ("ships", "GAMA_Railen", "player"),
        ("ships", "KRIG_S65_Stingray", "player"),
        ("items", "POWR_ACOM_S01_LumaCore_SCItem", "player"),
        ("items", "POWR_PyroOutpost", "special"),
    ] {
        assert_eq!(playability(&catalog, kind, class)?, expected, "{class}");
    }
    assert!(summary.ships.of(Playability::Player) > 0, "player ships");
    assert!(summary.ships.of(Playability::Ai) > 0, "ai ships");
    assert!(summary.items.of(Playability::Player) > 0, "player items");
    Ok(())
}

/// The Record tag the ship rule reads as an AI marker.
const AI_RECORD_TAG: &str = "SystemsDesign";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn no_player_ship_carries_the_ai_record_tag() -> TestResult {
    let archive =
        Archive::open_with_key(real_archive_path()?, p4kx_testkit::real_archive_key().ok())?;
    let built = build_catalog(
        &archive,
        CatalogOptions::new(SystemTime::now()).with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut tagged = 0_usize;
    for record in forge.records() {
        if record.tag()? != Some(AI_RECORD_TAG) {
            continue;
        }
        let Some((_, class)) = record.name()?.split_once('.') else {
            continue;
        };
        let Ok(value) = playability(&catalog, "ships", class) else {
            continue;
        };
        tagged += 1;
        assert_ne!(value, "player", "{class} carries the tag {AI_RECORD_TAG}");
    }
    assert!(
        tagged > 0,
        "no catalog ship carries the tag {AI_RECORD_TAG}"
    );
    Ok(())
}
