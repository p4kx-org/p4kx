// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The ship base data in the `ships` kind of the Basetool catalog, through the public API.

use crate::ship_base_fixture::{
    ALPHA, BETA, BROKEN, CAR, DISPLAY, GROUND, GUN_DISPLAY, HUGE, MIXED, ROLLING, TEMP, UNKNOWN,
    write_archive,
};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    CatalogOptions, Guid, SHIP_EXCLUDED_TOKENS, ShipBase, ShipBaseOptions, VehicleKind,
    build_catalog, build_ship_base, read_variant_groups,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);

fn catalog_options() -> CatalogOptions<'static> {
    CatalogOptions {
        publication_mode: p4kx_export::CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
    }
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

struct Built {
    catalog: Value,
    bases: Vec<ShipBase>,
}

fn build() -> Result<Built, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let built = build_catalog(&archive, &catalog_options())?;
    let bases = build_ship_base(&archive, &ShipBaseOptions::new(UNIX_EPOCH + EXPORTED_AT))?;
    Ok(Built {
        catalog: serde_json::from_slice(built.json())?,
        bases: bases.ships().to_vec(),
    })
}

fn ships(built: &Built) -> Result<&Vec<Value>, Box<dyn Error>> {
    built
        .catalog
        .get("ships")
        .and_then(Value::as_array)
        .ok_or_else(|| "the catalog has no ships".into())
}

fn ship(built: &Built, value: u128) -> Result<&Value, Box<dyn Error>> {
    let wanted = guid(value);
    ships(built)?
        .iter()
        .find(|ship| ship["guid"] == wanted.as_str())
        .ok_or_else(|| format!("no ship {wanted}").into())
}

#[test]
fn mass_hull_cargo_size_and_kind_come_from_the_ship_base_data() -> TestResult {
    let built = build()?;
    let alpha = ship(&built, ALPHA)?;
    assert_eq!(alpha["mass"], 1210.0);
    assert_eq!(alpha["massBasis"], "parts");
    assert_eq!(alpha["hullHealth"], 600.0);
    assert_eq!(
        alpha["cargoScu"], 18.5,
        "the docked snub craft's grid is not counted"
    );
    assert_eq!(alpha["cargoGridsWithIssue"], 0);
    assert_eq!(alpha["size"], 3);
    assert_eq!(alpha["vehicleKind"], "spaceship");
    let beta = ship(&built, BETA)?;
    assert_eq!(beta["mass"], 1270.0, "the Modification's patch is applied");
    assert_eq!(beta["hullHealth"], 650.0);
    Ok(())
}

#[test]
fn every_catalog_ship_equals_its_ship_base_data() -> TestResult {
    let built = build()?;
    assert_eq!(ships(&built)?.len(), built.bases.len());
    for base in &built.bases {
        let entry = ships(&built)?
            .iter()
            .find(|ship| ship["guid"] == base.guid.to_string().as_str())
            .ok_or_else(|| format!("{} is not in the catalog", base.class_name))?;
        let json = |value: Option<f64>| value.map_or(Value::Null, Value::from);
        assert_eq!(entry["mass"], json(base.mass.as_ref().map(|m| m.kg)));
        assert_eq!(
            entry["hullHealth"],
            json(base.hull.as_ref().map(|h| h.health))
        );
        let basis = match &base.mass {
            Some(mass) => serde_json::to_value(mass.basis)?,
            None => Value::Null,
        };
        assert_eq!(entry["massBasis"], basis, "{}", base.class_name);
        assert_eq!(entry["cargoScu"], json(base.cargo.scu));
        assert_eq!(entry["cargoGridsWithIssue"], base.cargo.grids_with_issue);
        assert_eq!(
            entry["size"],
            base.size.as_ref().map_or(Value::Null, |s| s.value.into())
        );
        assert_eq!(
            entry["vehicleKind"],
            base.vehicle_kind
                .map_or(Value::Null, |kind| kind.code().into()),
            "{}",
            base.class_name
        );
    }
    Ok(())
}

#[test]
fn a_value_that_cannot_be_determined_is_null_and_never_zero() -> TestResult {
    let built = build()?;
    let broken = ship(&built, BROKEN)?;
    assert!(broken["mass"].is_null() && broken["massBasis"].is_null());
    assert!(broken["hullHealth"].is_null() && broken["size"].is_null());
    let unknown = ship(&built, UNKNOWN)?;
    assert!(
        unknown["cargoScu"].is_null(),
        "grids without a readable capacity: unknown, not 0"
    );
    assert_eq!(unknown["cargoGridsWithIssue"], 2);
    assert!(unknown["hullHealth"].is_null());
    assert!(unknown["mass"].is_number());
    Ok(())
}

#[test]
fn a_part_value_too_large_to_round_is_unknown_and_never_null_with_a_basis() -> TestResult {
    let built = build()?;
    let huge = ship(&built, HUGE)?;
    assert_eq!(huge["mass"], 3000.0, "falls through to the physics mass");
    assert_eq!(huge["massBasis"], "physics");
    assert!(huge["hullHealth"].is_null());
    for ship in ships(&built)? {
        assert_eq!(ship["mass"].is_null(), ship["massBasis"].is_null());
    }
    Ok(())
}

#[test]
fn a_lower_bound_is_flagged_by_the_grids_with_an_issue() -> TestResult {
    let built = build()?;
    let mixed = ship(&built, MIXED)?;
    assert_eq!(mixed["cargoScu"], 16.0);
    assert_eq!(mixed["cargoGridsWithIssue"], 2);
    Ok(())
}

#[test]
fn a_ship_without_cargo_grids_has_zero_scu() -> TestResult {
    let built = build()?;
    let ground = ship(&built, GROUND)?;
    assert_eq!(ground["cargoScu"], 0.0);
    assert_eq!(ground["mass"], 5400.0);
    assert_eq!(ground["massBasis"], "physics");
    assert!(ground["hullHealth"].is_null());
    Ok(())
}

#[test]
fn the_kind_uses_the_wheels_of_the_implementation_as_evidence() -> TestResult {
    let built = build()?;
    assert_eq!(ship(&built, GROUND)?["vehicleKind"], "groundVehicle");
    assert_eq!(ship(&built, CAR)?["vehicleKind"], "groundVehicle");
    assert!(
        ship(&built, ROLLING)?["vehicleKind"].is_null(),
        "wheels under the spaceship folder"
    );
    assert_eq!(VehicleKind::Spaceship.code(), "spaceship");
    Ok(())
}

#[test]
fn a_temp_or_display_ship_is_left_out_everywhere_and_an_item_keeps_its_display_word() -> TestResult
{
    assert_eq!(SHIP_EXCLUDED_TOKENS, ["temp", "display", "hijacked"]);
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let built = build_catalog(&archive, &catalog_options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let bases = build_ship_base(&archive, &ShipBaseOptions::new(UNIX_EPOCH + EXPORTED_AT))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let groups = read_variant_groups(&archive, &forge, &|| false).ok_or("stopped")?;

    let classes = |kind: &str| -> Vec<String> {
        catalog[kind].as_array().map_or_default(|list| {
            list.iter()
                .filter_map(|entity| entity["className"].as_str().map(str::to_owned))
                .collect()
        })
    };
    let ships = classes("ships");
    for class in ["ACME_Alpha_Temp", "ACME_Beta_Display"] {
        assert!(
            !ships.iter().any(|ship| ship == class),
            "{class} in the catalog"
        );
        assert!(
            !bases.ships().iter().any(|ship| ship.class_name == class),
            "{class} in the ship base data"
        );
    }
    assert!(
        ships.iter().any(|ship| ship == "ACME_Alpha"),
        "the base hull stays"
    );
    assert!(
        classes("items")
            .iter()
            .any(|item| item == "ACME_Gun_Display"),
        "the item stays"
    );
    let skipped = built.summary().skipped();
    assert!(
        skipped >= 3,
        "two ships and the dummy are skipped: {skipped}"
    );
    for leftover in [TEMP, DISPLAY] {
        let guid: Guid = guid(leftover).parse()?;
        assert_eq!(groups.base_of(guid), None, "no variant group");
    }
    let alpha: Guid = guid(ALPHA).parse()?;
    let temp: Guid = guid(TEMP).parse()?;
    assert!(
        !groups.variants_of(alpha).contains(&temp),
        "not a variant of the base"
    );
    let item: Guid = guid(GUN_DISPLAY).parse()?;
    assert!(groups.base_of(item).is_none());
    Ok(())
}
