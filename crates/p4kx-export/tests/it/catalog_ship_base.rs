// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The ship base data in the `ships` kind of the P4KX catalog, through the public API.

#[path = "catalog_ship_physical_identity.rs"]
mod physical_identity;

#[path = "catalog_ship_aggregate_sources.rs"]
mod aggregate_sources;

#[path = "catalog_ship_cargo_resolution.rs"]
mod cargo_resolution;

#[path = "catalog_ship_exosuit_parity.rs"]
mod exosuit_parity;

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
    let built = build_catalog(&archive, catalog_options().with_full_provenance())?;
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

fn cargo<'catalog>(
    built: &'catalog Built,
    ship: &Value,
) -> Result<&'catalog Value, Box<dyn Error>> {
    let index = ship
        .pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .ok_or("physical Ship Record index")?;
    let rows = built
        .catalog
        .pointer("/projections/shipBaseFacts/rows")
        .and_then(Value::as_array)
        .ok_or("canonical Ship base facts")?;
    let mut matched = rows.iter().filter(|row| {
        row.pointer("/shipRecordIndex/value")
            .and_then(Value::as_u64)
            == Some(index)
    });
    let row = matched.next().ok_or("physical Ship base facts")?;
    assert!(
        matched.next().is_none(),
        "one result for each physical Ship"
    );
    assert_eq!(
        row["shipGuid"], ship["guid"],
        "the same original GUID and source"
    );
    row.get("cargo").ok_or_else(|| "canonical cargo".into())
}

fn ship(built: &Built, value: u128) -> Result<&Value, Box<dyn Error>> {
    let wanted = guid(value);
    ships(built)?
        .iter()
        .find(|ship| ship.pointer("/guid/value").and_then(Value::as_str) == Some(wanted.as_str()))
        .ok_or_else(|| format!("no ship {wanted}").into())
}

#[test]
fn mass_hull_cargo_size_and_kind_come_from_the_ship_base_data() -> TestResult {
    let built = build()?;
    let alpha = ship(&built, ALPHA)?;
    assert_eq!(alpha["mass"]["value"], 1210.0);
    assert_eq!(alpha["massBasis"]["value"], "parts");
    assert_eq!(alpha["hullHealth"]["value"], 600.0);
    assert_eq!(
        cargo(&built, alpha)?["scu"]["value"],
        18.5,
        "the docked snub craft's grid is not counted"
    );
    assert_eq!(cargo(&built, alpha)?["gridsWithIssue"]["value"], 0);
    assert_eq!(alpha["size"]["state"], "present");
    assert_eq!(
        alpha["size"]["value"],
        serde_json::json!({"type": "int32", "value": 3})
    );
    assert_eq!(alpha["vehicleKind"]["value"], "spaceship");
    let beta = ship(&built, BETA)?;
    assert_eq!(
        beta["mass"]["value"], 1270.0,
        "the Modification's patch is applied"
    );
    assert_eq!(beta["hullHealth"]["value"], 650.0);
    Ok(())
}

#[test]
fn every_catalog_ship_equals_its_ship_base_data() -> TestResult {
    let built = build()?;
    assert_eq!(ships(&built)?.len(), built.bases.len());
    for base in &built.bases {
        let guid = base.guid.to_string();
        let entry = ships(&built)?
            .iter()
            .find(|ship| ship.pointer("/guid/value").and_then(Value::as_str) == Some(guid.as_str()))
            .ok_or_else(|| format!("{} is not in the catalog", base.class_name))?;
        let json = |value: Option<f64>| value.map_or(Value::Null, Value::from);
        assert_eq!(
            entry["mass"]["value"],
            json(base.mass.as_ref().map(|m| m.kg))
        );
        assert_eq!(
            entry["hullHealth"]["value"],
            json(base.hull.as_ref().map(|h| h.health))
        );
        let basis = match &base.mass {
            Some(mass) => serde_json::to_value(mass.basis)?,
            None => Value::Null,
        };
        assert_eq!(entry["massBasis"]["value"], basis, "{}", base.class_name);
        let cargo = cargo(&built, entry)?;
        assert_eq!(cargo["scu"]["value"], json(base.cargo.scu));
        assert_eq!(
            cargo["gridsWithIssue"]["value"],
            base.cargo.grids_with_issue
        );
        let size = &entry["size"];
        if let Some(expected) = &base.size {
            assert_eq!(size["state"], "present");
            assert_eq!(
                size["value"],
                serde_json::json!({"type": "int32", "value": expected.value})
            );
        } else {
            assert_eq!(size["state"], "absent");
            assert!(size["value"].is_null(), "the ship has no authored size");
        }
        assert_eq!(
            entry["vehicleKind"]["value"],
            base.vehicle_kind
                .map_or(Value::Null, |kind| kind.code().into()),
            "{}",
            base.class_name
        );
    }
    Ok(())
}

#[test]
fn unavailable_aggregate_payloads_remain_null() -> TestResult {
    let built = build()?;
    let broken = ship(&built, BROKEN)?;
    assert!(broken["mass"]["value"].is_null() && broken["massBasis"]["value"].is_null());
    assert!(broken["hullHealth"]["value"].is_null());
    crate::health_origin_assertions::cell(broken, "size", "absent")?;
    let unknown = ship(&built, UNKNOWN)?;
    assert!(
        cargo(&built, unknown)?["scu"]["value"].is_null(),
        "grids without a readable capacity: unknown, not 0"
    );
    assert_eq!(cargo(&built, unknown)?["gridsWithIssue"]["value"], 2);
    assert!(unknown["hullHealth"]["value"].is_null());
    assert!(unknown["mass"]["value"].is_number());
    Ok(())
}

#[test]
fn a_part_value_too_large_to_round_is_unknown_and_never_null_with_a_basis() -> TestResult {
    let built = build()?;
    let huge = ship(&built, HUGE)?;
    assert_eq!(
        huge["mass"]["value"], 3000.0,
        "falls through to the physics mass"
    );
    assert_eq!(huge["massBasis"]["value"], "physics");
    assert!(huge["hullHealth"]["value"].is_null());
    for ship in ships(&built)? {
        assert_eq!(
            ship["mass"]["value"].is_null(),
            ship["massBasis"]["value"].is_null()
        );
    }
    Ok(())
}

#[test]
fn a_lower_bound_is_flagged_by_the_grids_with_an_issue() -> TestResult {
    let built = build()?;
    let mixed = ship(&built, MIXED)?;
    assert_eq!(cargo(&built, mixed)?["scu"]["value"], 16.0);
    assert_eq!(cargo(&built, mixed)?["gridsWithIssue"]["value"], 2);
    Ok(())
}

#[test]
fn a_ship_without_cargo_grids_has_zero_scu() -> TestResult {
    let built = build()?;
    let ground = ship(&built, GROUND)?;
    assert_eq!(cargo(&built, ground)?["scu"]["value"], 0.0);
    assert_eq!(ground["mass"]["value"], 5400.0);
    assert_eq!(ground["massBasis"]["value"], "physics");
    assert!(ground["hullHealth"]["value"].is_null());
    Ok(())
}

#[test]
fn the_kind_uses_the_wheels_of_the_implementation_as_evidence() -> TestResult {
    let built = build()?;
    assert_eq!(
        ship(&built, GROUND)?["vehicleKind"]["value"],
        "groundVehicle"
    );
    assert_eq!(ship(&built, CAR)?["vehicleKind"]["value"], "groundVehicle");
    assert!(
        ship(&built, ROLLING)?["vehicleKind"]["value"].is_null(),
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
    let built = build_catalog(&archive, catalog_options().with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let bases = build_ship_base(&archive, &ShipBaseOptions::new(UNIX_EPOCH + EXPORTED_AT))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let groups = read_variant_groups(&archive, &forge, &|| false).ok_or("stopped")?;

    let classes = |kind: &str| -> Vec<String> {
        catalog[kind].as_array().map_or_default(|list| {
            list.iter()
                .filter_map(|entity| {
                    crate::catalog_items::entity_field(entity, kind, "className")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
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

#[path = "catalog_ship_source_walk.rs"]
pub(crate) mod source_sdk_walk;

#[path = "catalog_ship_source_arena_coverage.rs"]
mod source_arena_coverage;
