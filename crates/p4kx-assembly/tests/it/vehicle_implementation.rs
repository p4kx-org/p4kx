// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The vehicle implementation of an Entity: per-port visibility, treads, wheels and landing gear
//! in the Loadout tree.

use crate::vehicle_support;

use std::collections::HashMap;

use p4kx_assembly::{
    LoadoutError, LoadoutResolver, PartKind, PortVisibility, VehicleImplementation,
};
use p4kx_dataforge::DataForge;
use p4kx_testkit::{
    CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntity, entity_schema,
};
use vehicle_support::{
    DEFINITION, Files, TestResult, files, part_names, resolve, standard_files, visibility,
};

#[test]
fn the_base_implementation_gives_per_port_visibility_ignoring_case() -> TestResult {
    let tree = resolve(DEFINITION, "", Some(&standard_files()))?;
    assert!(tree.issues().is_empty(), "issues: {:?}", tree.issues());
    assert_eq!(
        visibility(&tree),
        [
            ("hardpoint_gun".to_owned(), PortVisibility::Invisible),
            ("hardpoint_seat".to_owned(), PortVisibility::Visible),
            ("hardpoint_unnamed".to_owned(), PortVisibility::Unspecified),
        ],
        "the part Hardpoint_Gun decides the port hardpoint_gun"
    );
    Ok(())
}

#[test]
fn skipped_parts_are_left_out_and_numbers_are_read_leniently() -> TestResult {
    let tree = resolve(DEFINITION, "", Some(&standard_files()))?;
    let vehicle = tree.vehicle().ok_or("no vehicle")?;
    assert_eq!(vehicle.name(), "TEST_Ship");
    assert_eq!(vehicle.path(), DEFINITION);
    assert_eq!(vehicle.modification(), None);
    assert_eq!(
        part_names(vehicle.item_ports()),
        ["Hardpoint_Gun", "hardpoint_seat"],
        "hardpoint_extra is skipPart"
    );
    let gun = vehicle.item_ports().next().ok_or("no port part")?;
    assert_eq!(gun.damage_max(), Some(120.0));
    let port = gun.item_port().ok_or("no item port")?;
    assert_eq!((port.min_size(), port.max_size()), (Some(1), Some(3)));
    assert!(port.has_flag("uneditable") && port.has_flag("INVISIBLE"));
    let seat = vehicle.item_ports().nth(1).ok_or("no seat")?;
    let seat = seat.item_port().ok_or("no item port")?;
    assert_eq!(
        (seat.min_size(), seat.max_size()),
        (Some(2), Some(2)),
        "attribute names ignore case"
    );
    let body = vehicle.parts().first().ok_or("no body")?;
    assert_eq!(body.mass(), Some(1500.0));
    assert_eq!(body.kind(), PartKind::Other);
    let masses: Vec<_> = vehicle
        .wheels()
        .map(p4kx_assembly::VehiclePart::mass)
        .collect();
    assert_eq!(masses, [Some(12.5), None], "stray characters are ignored");
    Ok(())
}

#[test]
fn the_implementation_can_be_read_without_a_loadout_tree() -> TestResult {
    let files = standard_files();
    let base = VehicleImplementation::read(&files, "TEST_Ship", DEFINITION, None)?;
    let open = VehicleImplementation::read(&files, "TEST_Ship", DEFINITION, Some("Open"))?;
    assert_eq!(
        base.port_visibility("hardpoint_gun"),
        PortVisibility::Invisible
    );
    assert_eq!(
        open.port_visibility("HARDPOINT_GUN"),
        PortVisibility::Visible
    );
    let missing = VehicleImplementation::read(&files, "OTHER_Ship", "nope.xml", None)
        .err()
        .ok_or("the configured absent file was accepted")?;
    assert!(
        matches!(&missing, LoadoutError::MissingVehicleImplementation { path } if path == "nope.xml"),
        "a missing configured implementation retains its typed cause and exact path"
    );
    assert_eq!(
        missing.kind(),
        "vehicle_implementation",
        "the typed missing cause retains the public script code"
    );
    Ok(())
}

#[test]
fn treads_list_their_wheels_and_wheels_are_listed() -> TestResult {
    let tree = resolve(DEFINITION, "", Some(&standard_files()))?;
    let vehicle = tree.vehicle().ok_or("no vehicle")?;
    assert_eq!(part_names(vehicle.treads()), ["left_tread"]);
    let tread = vehicle.treads().next().ok_or("no tread")?;
    assert_eq!(tread.tread_wheels(), ["wheel_l1", "wheel_l2"]);
    assert_eq!(part_names(vehicle.wheels()), ["wheel_l1", "wheel_l2"]);
    Ok(())
}

#[test]
fn the_landing_gear_of_the_referenced_record_is_listed() -> TestResult {
    let tree = resolve(DEFINITION, "", Some(&standard_files()))?;
    let gear: Vec<_> = tree
        .landing_gear()
        .iter()
        .map(|gear| (gear.bone(), gear.geometry()))
        .collect();
    assert_eq!(
        gear,
        [
            ("landing_gear_front", "Objects/gear_front.cgf"),
            ("landing_gear_back", "Objects/gear_back.cgf")
        ]
    );
    let masses: Vec<_> = tree
        .landing_gear()
        .iter()
        .map(p4kx_assembly::LandingGear::mass)
        .collect();
    assert_eq!(masses, [Some(10.0), Some(12.5)]);
    Ok(())
}

#[test]
fn a_definition_the_archive_lacks_falls_back_to_the_implementation_folder() -> TestResult {
    let tree = resolve(
        "Scripts/Entities/Vehicles/Implementations/Xml/Old/test_ship.xml",
        "",
        Some(&standard_files()),
    )?;
    assert!(tree.issues().is_empty(), "issues: {:?}", tree.issues());
    let vehicle = tree.vehicle().ok_or("no vehicle")?;
    assert!(
        vehicle.path().eq_ignore_ascii_case(DEFINITION),
        "path {}",
        vehicle.path()
    );
    Ok(())
}

#[test]
fn an_unreadable_implementation_is_an_issue_of_the_entity() -> TestResult {
    let broken = files(&[(DEFINITION, "<Vehicle><Parts>")]);
    let wrong_root = files(&[(DEFINITION, "<Loadout/>")]);
    for files in [&broken, &wrong_root, &files(&[])] {
        let tree = resolve(DEFINITION, "", Some(files))?;
        assert!(tree.vehicle().is_none());
        let kinds: Vec<_> = tree.issues().iter().map(LoadoutError::kind).collect();
        assert_eq!(kinds, ["vehicle_implementation"]);
        assert!(
            tree.ports()
                .iter()
                .all(|port| port.visibility() == PortVisibility::Unspecified),
            "no visibility without an implementation"
        );
    }
    Ok(())
}

#[test]
fn an_empty_definition_means_the_entity_has_no_implementation_and_no_issue() -> TestResult {
    let tree = resolve("", "", Some(&standard_files()))?;
    assert!(tree.vehicle().is_none());
    assert!(tree.issues().is_empty(), "issues: {:?}", tree.issues());
    Ok(())
}

#[test]
fn without_files_the_entity_has_no_implementation_and_no_issue() -> TestResult {
    let tree = resolve(DEFINITION, "Open", None)?;
    assert!(tree.vehicle().is_none());
    assert!(tree.issues().is_empty(), "issues: {:?}", tree.issues());
    Ok(())
}

#[test]
fn a_cryxml_implementation_is_read_like_text() -> TestResult {
    let part = |name: &str, flags: &str| {
        TestElement::new("Part")
            .attribute("name", name)
            .attribute("class", "ItemPort")
            .child(TestElement::new("ItemPort").attribute("flags", flags))
    };
    let vehicle = TestElement::new("Vehicle")
        .attribute("name", "TEST_Ship")
        .child(
            TestElement::new("Parts").child(
                TestElement::new("Part").attribute("name", "body").child(
                    TestElement::new("Parts")
                        .child(part("hardpoint_gun", "invisible"))
                        .child(part("hardpoint_seat", "")),
                ),
            ),
        );
    let bytes = vehicle.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let files = Files(HashMap::from([(DEFINITION.to_ascii_lowercase(), bytes)]));
    let tree = resolve(DEFINITION, "", Some(&files))?;
    assert!(tree.issues().is_empty(), "issues: {:?}", tree.issues());
    assert_eq!(
        visibility(&tree)
            .into_iter()
            .map(|(_, visibility)| visibility)
            .collect::<Vec<_>>(),
        [
            PortVisibility::Invisible,
            PortVisibility::Visible,
            PortVisibility::Unspecified
        ]
    );
    Ok(())
}

#[test]
fn an_implementation_nested_too_deep_is_refused() -> TestResult {
    let depth = 200;
    let text = format!(
        "<Vehicle>{}{}</Vehicle>",
        "<Parts>".repeat(depth),
        "</Parts>".repeat(depth)
    );
    let tree = resolve(DEFINITION, "", Some(&files(&[(DEFINITION, &text)])))?;
    assert!(tree.vehicle().is_none());
    assert_eq!(tree.issues().len(), 1, "issues: {:?}", tree.issues());
    Ok(())
}

#[test]
fn a_landing_system_that_resolves_to_no_record_is_an_issue() -> TestResult {
    let ship = TestEntity::new(0x1, "TEST_Ship", "entities/test_ship.xml")
        .vehicle(DEFINITION, "")
        .landing_system(0x99);
    let bytes = entity_schema(DataForgeBuilder::new())
        .record(ship.record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    let entity = resolver.entities().find("TEST_Ship")?;
    let tree = resolver.resolve(&entity)?;
    assert_eq!(tree.landing_gear().len(), 0);
    let kinds: Vec<_> = tree.issues().iter().map(LoadoutError::kind).collect();
    assert_eq!(kinds, ["dangling_reference"]);
    Ok(())
}
