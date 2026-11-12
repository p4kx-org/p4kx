// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The base data of ships through the public API.

#[path = "ship_base_cargo_bounds.rs"]
mod cargo_bounds;

use crate::ship_base_fixture::{
    ALPHA, ALPHA_XML, BETA, BROKEN, CAR, GROUND, MIXED, ROLLING, UNKNOWN, write_archive,
};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    BuiltShipBase, Filter, MassBasis, ShipBase, ShipBaseOptions, SourceKind, VehicleKind,
    build_ship_base,
};
use p4kx_extraction::Archive;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

fn build(filter: Option<&Filter>) -> Result<BuiltShipBase, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let mut options = ShipBaseOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.filter = filter;
    Ok(build_ship_base(&archive, &options)?)
}

fn ship(built: &BuiltShipBase, guid: u128) -> Result<&ShipBase, Box<dyn Error>> {
    let text = format!("{guid:032x}");
    let wanted = format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    );
    built
        .ships()
        .iter()
        .find(|ship| ship.guid.to_string() == wanted)
        .ok_or_else(|| format!("no ship {wanted}").into())
}

fn near(left: f64, right: f64) -> bool {
    (left - right).abs() < 1e-6
}

#[test]
fn ships_are_the_catalog_ships_without_development_leftovers() -> TestResult {
    let built = build(None)?;
    assert_eq!(built.ships().len(), 9, "failures: {:?}", built.failures());
    assert_eq!(
        built.skipped(),
        3,
        "ACME_Dummy_Ship holds the token dummy, the other two a ship word"
    );
    assert!(built.is_complete(), "failures: {:?}", built.failures());
    let order: Vec<_> = built
        .ships()
        .iter()
        .map(|s| s.class_name.as_str())
        .collect();
    assert_eq!(
        order,
        [
            "ACME_Alpha",
            "ACME_Beta",
            "ACME_Ground",
            "ACME_Broken",
            "ACME_Mixed",
            "ACME_Unknown",
            "ACME_Rolling",
            "ACME_Car",
            "ACME_Huge"
        ],
        "ordered by the text of the GUID"
    );
    Ok(())
}

#[test]
fn the_vehicle_kind_follows_the_folder_and_the_wheels_of_the_implementation() -> TestResult {
    let built = build(None)?;
    assert_eq!(
        ship(&built, ALPHA)?.vehicle_kind,
        Some(VehicleKind::Spaceship)
    );
    assert_eq!(
        ship(&built, GROUND)?.vehicle_kind,
        Some(VehicleKind::GroundVehicle),
        "the ground folder is enough without an implementation"
    );
    assert_eq!(
        ship(&built, CAR)?.vehicle_kind,
        Some(VehicleKind::GroundVehicle)
    );
    assert_eq!(
        ship(&built, ROLLING)?.vehicle_kind,
        None,
        "wheels in a spaceship folder: the evidence disagrees"
    );
    assert_eq!(VehicleKind::GroundVehicle.code(), "groundVehicle");
    Ok(())
}

#[test]
fn the_code_of_a_vehicle_kind_is_its_serialised_name() -> TestResult {
    for kind in [VehicleKind::Spaceship, VehicleKind::GroundVehicle] {
        assert_eq!(serde_json::to_value(kind)?, kind.code());
    }
    Ok(())
}

#[test]
fn a_mass_basis_serialises_to_the_names_the_catalog_documents() -> TestResult {
    assert_eq!(serde_json::to_value(MassBasis::Parts)?, "parts");
    assert_eq!(serde_json::to_value(MassBasis::Physics)?, "physics");
    Ok(())
}

#[test]
fn the_size_comes_from_the_attach_definition_with_its_source() -> TestResult {
    let built = build(None)?;
    let alpha = ship(&built, ALPHA)?;
    let size = alpha.size.as_ref().ok_or("no size")?;
    assert_eq!(size.value, 3);
    assert_eq!(size.source.kind, SourceKind::Record);
    assert_eq!(size.source.file, alpha.record_path);
    assert_eq!(
        size.source.field,
        "Components/SAttachableComponentParams/AttachDef/Size"
    );
    assert!(
        ship(&built, GROUND)?.size.is_none(),
        "the ground vehicle has no AttachDef"
    );
    Ok(())
}

#[test]
fn mass_and_hull_are_summed_over_the_parts_with_their_sources() -> TestResult {
    let built = build(None)?;
    let alpha = ship(&built, ALPHA)?;
    let mass = alpha.mass.as_ref().ok_or("no mass")?;
    assert!(near(mass.kg, 1210.0), "1000 + 200 + 10: {}", mass.kg);
    assert_eq!(mass.basis, MassBasis::Parts);
    assert_eq!(mass.parts, 3);
    assert_eq!(mass.sources.len(), 1);
    assert_eq!(mass.sources[0].source.kind, SourceKind::VehicleXml);
    assert!(mass.sources[0].source.file.ends_with("ACME_Alpha.xml"));
    assert_eq!(mass.sources[0].values, 3);
    let hull = alpha.hull.as_ref().ok_or("no hull")?;
    assert!(
        near(hull.health, 600.0),
        "the uneditable hardpoint is left out"
    );
    assert_eq!(hull.parts, 2);
    assert_eq!(hull.destruction.len(), 1);
    assert_eq!(hull.destruction[0].part, "hull");
    assert!(near(hull.destruction[0].damage, 250.0));
    assert_eq!(hull.detach.len(), 1);
    assert_eq!(hull.detach[0].part, "wing");
    assert!(near(hull.detach[0].damage, 25.0));
    let implementation = alpha.implementation.as_ref().ok_or("no implementation")?;
    assert!(implementation.file.ends_with("ACME_Alpha.xml"));
    assert_eq!(implementation.patch_file, None);
    assert!(
        ALPHA_XML.contains("Heavy"),
        "the fixture states the Modification"
    );
    Ok(())
}

#[test]
fn the_modification_moves_values_and_names_the_patch_and_the_elem() -> TestResult {
    let built = build(None)?;
    let beta = ship(&built, BETA)?;
    let mass = beta.mass.as_ref().ok_or("no mass")?;
    assert!(
        near(mass.kg, 1270.0),
        "the patch raised the wing to 260: {}",
        mass.kg
    );
    let kinds: Vec<_> = mass
        .sources
        .iter()
        .map(|share| (share.source.kind, share.values))
        .collect();
    assert_eq!(
        kinds,
        [(SourceKind::VehicleXml, 2), (SourceKind::PatchXml, 1)]
    );
    assert!(
        mass.sources[1]
            .source
            .file
            .ends_with("Modifications/ACME_Alpha_Heavy.xml")
    );
    let hull = beta.hull.as_ref().ok_or("no hull")?;
    assert!(
        near(hull.health, 650.0),
        "the Elem set the wing's damageMax to 150"
    );
    let kinds: Vec<_> = hull
        .sources
        .iter()
        .map(|share| (share.source.kind, share.values))
        .collect();
    assert_eq!(
        kinds,
        [(SourceKind::VehicleXml, 1), (SourceKind::Modification, 1)]
    );
    assert!(hull.sources[1].source.field.contains("Heavy"));
    assert_eq!(hull.detach[0].origin, SourceKind::Modification);
    assert!(near(hull.detach[0].damage, 37.5));
    Ok(())
}

#[test]
fn cargo_sums_the_grids_of_the_loadout_but_not_templates_or_docked_vehicles() -> TestResult {
    let built = build(None)?;
    let cargo = &ship(&built, ALPHA)?.cargo;
    let scu = cargo.scu.ok_or("no scu")?;
    assert!(near(scu, 18.5), "16 SCU open grid + 2.5 SCU closed: {scu}");
    assert_eq!(cargo.cells, Some(16));
    assert_eq!(cargo.grids_with_issue, 0);
    let ports: Vec<_> = cargo.grids.iter().map(|grid| grid.port.as_str()).collect();
    assert_eq!(ports, ["grid_main", "grid_aux"]);
    let open = &cargo.grids[0];
    assert_eq!(open.shape, Some("open"));
    assert_eq!(open.dimensions, Some([2.5, 5.0, 2.5]));
    assert_eq!(
        open.cells,
        Some(16),
        "floor(2.5 / 1.25) * floor(5 / 1.25) * floor(2.5 / 1.25)"
    );
    assert!(
        open.source
            .as_ref()
            .is_some_and(|s| s.field == "interiorDimensions")
    );
    let closed = &cargo.grids[1];
    assert_eq!(closed.shape, Some("closed"));
    assert_eq!(closed.scu, Some(2.5), "250 centi SCU");
    assert_eq!(closed.cells, None);
    assert!(
        closed
            .source
            .as_ref()
            .is_some_and(|s| s.field.ends_with("SCentiCargoUnit/centiSCU"))
    );
    let none = ship(&built, GROUND)?.cargo.scu.ok_or("no grids is 0")?;
    assert!(
        near(none, 0.0) && none.is_sign_positive(),
        "no negative zero: {none}"
    );
    Ok(())
}

#[test]
fn nonfinite_or_negative_dimensions_are_unreadable() -> TestResult {
    let built = build(None)?;
    let mixed = &ship(&built, MIXED)?.cargo;
    assert_eq!(mixed.grids_with_issue, 2);
    assert!(near(mixed.scu.ok_or("one grid is readable")?, 16.0));
    assert_eq!(mixed.cells, Some(16));
    for grid in &mixed.grids[..2] {
        assert!(
            grid.scu.is_none() && grid.cells.is_none() && grid.issue.is_some(),
            "{}: {:?}",
            grid.port,
            grid.issue
        );
    }
    let unknown = ship(&built, UNKNOWN)?;
    assert_eq!(unknown.cargo.scu, None, "unknown cargo is not zero cargo");
    assert_eq!(unknown.cargo.cells, None);
    assert_eq!(unknown.cargo.grids_with_issue, 2);
    assert!(unknown.hull.is_none(), "no part states a damageMax");
    assert!(unknown.mass.is_some());
    Ok(())
}

#[test]
fn the_loadout_lists_class_names_per_port_path_and_marks_docked_vehicles() -> TestResult {
    let built = build(None)?;
    let alpha = ship(&built, ALPHA)?;
    let rows: Vec<_> = alpha
        .loadout
        .iter()
        .map(|row| {
            (
                row.port.as_str(),
                row.class_name.as_deref(),
                row.docked_vehicle,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("grid_main", Some("ACME_Grid_Open"), false),
            ("grid_aux", Some("ACME_Grid_Closed"), false),
            ("grid_old", Some("ACME_Grid_template"), false),
            ("gun", Some("ACME_Gun"), false),
            ("hangar", Some("ACME_Snub"), true),
            ("lost", Some("NoSuchClass"), false),
        ],
        "the snub craft stays in its port and is not expanded"
    );
    assert!(alpha.loadout[5].unresolved.is_some());
    assert_eq!(alpha.loadout_source.kind, SourceKind::Record);
    Ok(())
}

#[test]
fn a_ground_vehicle_falls_back_to_the_physics_mass() -> TestResult {
    let built = build(None)?;
    let ground = ship(&built, GROUND)?;
    let mass = ground.mass.as_ref().ok_or("no mass")?;
    assert!(near(mass.kg, 5400.0));
    assert_eq!(mass.basis, MassBasis::Physics);
    assert_eq!(
        mass.sources[0].source.field,
        "Components/SEntityPhysicsControllerParams/PhysType/Mass"
    );
    assert!(ground.hull.is_none() && ground.implementation.is_none());
    Ok(())
}

#[test]
fn an_unreadable_implementation_is_an_issue_and_leaves_the_values_out() -> TestResult {
    let built = build(None)?;
    let broken = ship(&built, BROKEN)?;
    assert!(broken.mass.is_none() && broken.hull.is_none());
    assert!(
        broken
            .issues
            .iter()
            .any(|issue| issue.contains("ACME_Broken")),
        "issues: {:?}",
        broken.issues
    );
    Ok(())
}

#[test]
fn json_and_ndjson_carry_the_same_ships_in_the_same_order() -> TestResult {
    let built = build(None)?;
    let mut json = Vec::new();
    built.write_json(&mut json)?;
    let document: Value = serde_json::from_slice(&json)?;
    assert_eq!(document["schemaVersion"], 2);
    assert_eq!(document["filter"], Value::Null);
    assert_eq!(
        document["source"]["exportedAt"].as_str().map(str::len),
        Some(20)
    );
    let from_json: Vec<Value> = document["ships"].as_array().ok_or("ships")?.clone();
    let mut ndjson = Vec::new();
    built.write_ndjson(&mut ndjson)?;
    let lines: Vec<Value> = std::str::from_utf8(&ndjson)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    assert_eq!(lines, from_json);
    assert_eq!(lines[0]["cargo"]["scu"], 18.5);
    assert!(lines[5]["cargo"]["scu"].is_null(), "unknown cargo is null");
    assert_eq!(lines[0]["mass"]["sources"][0]["kind"], "vehicleXml");
    assert_eq!(lines[0]["loadout"][4]["dockedVehicle"], true);
    assert!(lines[0]["loadout"][0].get("dockedVehicle").is_none());
    Ok(())
}

#[test]
fn a_filter_narrows_the_ships_and_marks_the_export_partial() -> TestResult {
    let filter = Filter::new("acme_alpha");
    let built = build(Some(&filter))?;
    assert_eq!(built.ships().len(), 1);
    assert!(!built.is_complete());
    let mut json = Vec::new();
    built.write_json(&mut json)?;
    let document: Value = serde_json::from_slice(&json)?;
    assert_eq!(document["filter"], "acme_alpha");
    Ok(())
}
