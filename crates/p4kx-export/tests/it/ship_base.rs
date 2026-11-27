// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The base data of ships through the public API.

#[path = "ship_base_cargo_bounds.rs"]
mod cargo_bounds;
#[path = "ship_base_provenance.rs"]
mod provenance;

use crate::ship_base_fixture::{
    self as fixture, ALPHA, ALPHA_XML, BETA, BROKEN, CAR, GROUND, MIXED, ROLLING, UNKNOWN,
};

use p4kx_export::{Filter, MassBasis, SourceKind, VehicleKind};
use serde_json::Value;
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

fn near(left: f64, right: f64) -> bool {
    (left - right).abs() < 1e-6
}

#[test]
fn ships_are_the_catalog_ships_without_development_leftovers() -> TestResult {
    let built = fixture::build(None)?;
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
    let built = fixture::build(None)?;
    assert_eq!(
        fixture::ship_by_guid(&built, ALPHA)?.vehicle_kind,
        Some(VehicleKind::Spaceship)
    );
    assert_eq!(
        fixture::ship_by_guid(&built, GROUND)?.vehicle_kind,
        Some(VehicleKind::GroundVehicle),
        "the ground folder is enough without an implementation"
    );
    assert_eq!(
        fixture::ship_by_guid(&built, CAR)?.vehicle_kind,
        Some(VehicleKind::GroundVehicle)
    );
    assert_eq!(
        fixture::ship_by_guid(&built, ROLLING)?.vehicle_kind,
        None,
        "wheels in a spaceship folder: the evidence disagrees"
    );
    assert_eq!(VehicleKind::GroundVehicle.code(), "groundVehicle");
    Ok(())
}

#[test]
fn the_code_of_a_vehicle_kind_is_its_serialised_name() -> TestResult {
    for kind in [
        VehicleKind::Spaceship,
        VehicleKind::GroundVehicle,
        VehicleKind::Exosuit,
    ] {
        assert_eq!(serde_json::to_value(kind)?, kind.code());
    }
    assert_eq!(VehicleKind::Exosuit.code(), "exosuit");
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
    let built = fixture::build(None)?;
    let alpha = fixture::ship_by_guid(&built, ALPHA)?;
    let size = alpha.size.as_ref().ok_or("no size")?;
    assert_eq!(size.value, 3);
    assert_eq!(size.source.kind, SourceKind::Record);
    assert_eq!(size.source.file, alpha.record_path);
    assert_eq!(
        size.source.field,
        "Components/SAttachableComponentParams/AttachDef/Size"
    );
    assert!(
        fixture::ship_by_guid(&built, GROUND)?.size.is_none(),
        "the ground vehicle has no AttachDef"
    );
    Ok(())
}

#[test]
fn mass_and_hull_are_summed_over_the_parts_with_their_sources() -> TestResult {
    let built = fixture::build(None)?;
    let alpha = fixture::ship_by_guid(&built, ALPHA)?;
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
    let built = fixture::build(None)?;
    let beta = fixture::ship_by_guid(&built, BETA)?;
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
    let built = fixture::build(None)?;
    let cargo = &fixture::ship_by_guid(&built, ALPHA)?.cargo;
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
    let none = fixture::ship_by_guid(&built, GROUND)?
        .cargo
        .scu
        .ok_or("no grids is 0")?;
    assert!(
        near(none, 0.0) && none.is_sign_positive(),
        "no negative zero: {none}"
    );
    Ok(())
}

#[test]
fn nonfinite_or_negative_dimensions_are_unreadable() -> TestResult {
    let built = fixture::build(None)?;
    let mixed = &fixture::ship_by_guid(&built, MIXED)?.cargo;
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
    let unknown = fixture::ship_by_guid(&built, UNKNOWN)?;
    assert_eq!(unknown.cargo.scu, None, "unknown cargo is not zero cargo");
    assert_eq!(unknown.cargo.cells, None);
    assert_eq!(unknown.cargo.grids_with_issue, 2);
    assert!(unknown.hull.is_none(), "no part states a damageMax");
    assert!(unknown.mass.is_some());
    Ok(())
}

#[test]
fn the_loadout_lists_class_names_per_port_path_and_marks_docked_vehicles() -> TestResult {
    let built = fixture::build(None)?;
    let alpha = fixture::ship_by_guid(&built, ALPHA)?;
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
    let built = fixture::build(None)?;
    let ground = fixture::ship_by_guid(&built, GROUND)?;
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
    let built = fixture::build(None)?;
    let broken = fixture::ship_by_guid(&built, BROKEN)?;
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
    let built = fixture::build(None)?;
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
    assert_eq!(lines.len(), from_json.len(), "same number of vehicle rows");
    for (line, expected) in lines.iter().zip(&from_json) {
        let mut value = line.clone();
        let fields = value.as_object_mut().ok_or("NDJSON row object")?;
        assert!(
            fields.remove("provenance").is_some(),
            "the line carries its own source closure"
        );
        assert_eq!(
            &value, expected,
            "legacy data and row evidence are identical across formats"
        );
    }
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
    let built = fixture::build(Some(&filter))?;
    assert_eq!(built.ships().len(), 1);
    assert!(!built.is_complete());
    let mut json = Vec::new();
    built.write_json(&mut json)?;
    let document: Value = serde_json::from_slice(&json)?;
    assert_eq!(document["filter"], "acme_alpha");
    Ok(())
}
