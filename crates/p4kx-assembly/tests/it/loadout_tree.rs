// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Loadout tree of the synthetic sample ship: ports, attached Entities, sources, port
//! definitions and the ports that cannot be resolved.

use std::error::Error;

use p4kx_assembly::{
    Attachment, EntityNode, LoadoutError, LoadoutResolver, LoadoutTargetGuid, PortDefinition,
    PortSource, Target,
};
use p4kx_dataforge::{DataForge, Guid};
use p4kx_testkit::sample_ship;

type TestResult = Result<(), Box<dyn Error>>;

fn ship_tree() -> Result<EntityNode, Box<dyn Error>> {
    let bytes = sample_ship().build();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    let ship = resolver.entities().find("TEST_Ship")?;
    Ok(resolver.resolve(&ship)?)
}

fn summary(node: &EntityNode, depth: usize, lines: &mut Vec<String>) {
    for port in node.ports() {
        let indent = "  ".repeat(depth);
        let source = format!("{:?}", port.source());
        match port.attachment() {
            Attachment::Empty => lines.push(format!("{indent}{} [{source}] -", port.name())),
            Attachment::Entity(child) => {
                lines.push(format!(
                    "{indent}{} [{source}] {}",
                    port.name(),
                    child.class_name()
                ));
                summary(child, depth + 1, lines);
            }
            Attachment::Unresolved(unresolved) => lines.push(format!(
                "{indent}{} [{source}] ! {}",
                port.name(),
                unresolved.error().kind()
            )),
        }
    }
}

#[test]
fn the_tree_lists_ports_with_attached_entities_recursively() -> TestResult {
    let tree = ship_tree()?;
    assert_eq!(tree.class_name(), "TEST_Ship", "class name");
    assert_eq!(
        tree.name(),
        "EntityClassDefinition.TEST_Ship",
        "Record name"
    );
    assert_eq!(
        tree.record_path(),
        "libs/foundry/records/entities/spaceships/test_ship",
        "record path"
    );
    let mut lines = Vec::new();
    summary(&tree, 0, &mut lines);
    assert_eq!(
        lines,
        [
            "hardpoint_paint [Loadout] -",
            "hardpoint_seat_pilot [Loadout] TEST_Seat",
            "hardpoint_dashboard [Loadout] TEST_Dashboard",
            "  Display_HUD [Loadout] TEST_Screen",
            "  Radar [Override] TEST_Radar_Screen",
            "hardpoint_weapon_nose [Loadout] TEST_Mount_Gimbal_S3",
            "  hardpoint_class_2 [Override] TEST_Gun_S3",
            "hardpoint_cargo [Loadout] ! dangling_reference",
            "hardpoint_door [Loadout] ! unknown_class",
            "hardpoint_relay [Unset] -",
        ],
        "ports in Loadout order, then the defined ports no Loadout names"
    );
    Ok(())
}

#[test]
fn unresolvable_ports_keep_their_target_and_reason() -> TestResult {
    let tree = ship_tree()?;
    let unresolved: Vec<_> = tree.unresolved().collect();
    assert_eq!(unresolved.len(), 2, "two ports cannot be resolved");
    let missing: Guid = "00000001-0002-0003-0004-000000000005".parse()?;
    let (path, cargo) = unresolved.first().ok_or("no first")?;
    assert_eq!(path, "hardpoint_cargo", "port path");
    assert_eq!(cargo.target(), &Target::Reference(missing), "target");
    assert_eq!(
        cargo.error(),
        &LoadoutError::DanglingReference {
            guid: LoadoutTargetGuid::unbound(missing)
        },
        "reason"
    );
    let (path, door) = unresolved.get(1).ok_or("no second")?;
    assert_eq!(path, "hardpoint_door", "port path");
    assert_eq!(
        door.target(),
        &Target::ClassName("TEST_No_Such_Door".to_owned()),
        "target"
    );
    assert_eq!(
        door.error().to_string(),
        "no Entity has the class name TEST_No_Such_Door",
        "reason"
    );
    Ok(())
}

#[test]
fn a_port_definition_gives_sizes_types_and_transform() -> TestResult {
    let tree = ship_tree()?;
    let nose = tree
        .ports()
        .iter()
        .find(|port| port.name() == "hardpoint_weapon_nose")
        .ok_or("no nose port")?;
    let definition = nose.definition().ok_or("no definition")?;
    assert_eq!(definition.min_size(), Some(2), "min size");
    assert_eq!(definition.max_size(), Some(3), "max size");
    let types: Vec<_> = definition
        .types()
        .iter()
        .map(|item_type| (item_type.name(), item_type.sub_types().to_vec()))
        .collect();
    assert_eq!(types, [("WeaponGun", vec!["Gun".to_owned()])], "types");
    let transform = definition.transform().ok_or("no transform")?;
    assert_eq!(transform.attachment(), "Bone", "attachment kind");
    assert_eq!(transform.helper(), "hp_nose", "helper");
    assert_eq!(transform.position(), [0.0, 4.5, -0.25], "position");
    assert_eq!(transform.rotation(), [0.0, 0.0, 90.0], "rotation");
    let relay = tree.ports().last().ok_or("no ports")?;
    assert_eq!(relay.source(), PortSource::Unset, "defined but unset");
    assert_eq!(
        relay.definition().and_then(PortDefinition::min_size),
        Some(0),
        "relay"
    );
    Ok(())
}
