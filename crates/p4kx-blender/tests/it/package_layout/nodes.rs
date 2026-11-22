// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_blender::{Node, NodeKind};
use p4kx_testkit::repeated_gimbal_assembly_entries;

use super::{write, write_entries};
use crate::support::TestResult;

#[test]
fn nodes_keep_the_hierarchy_and_names_of_the_assembly() -> TestResult {
    let written = write(0)?;
    let nodes = &written.manifest.nodes;

    let names: Vec<&str> = nodes.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "TEST_Livery",
            "hardpoint_painted/TEST_Painted",
            "hardpoint_twin/TEST_Painted",
            "hardpoint_default/TEST_Default",
            "hardpoint_lost/TEST_Lost",
            "hardpoint_child/TEST_Child",
            "hardpoint_plain/TEST_Plain",
        ]
    );
    assert!(nodes.iter().all(|node| node.kind == NodeKind::Entity));
    assert!(nodes.iter().all(|node| node.entity.is_some()));
    assert_eq!(nodes.first().and_then(|node| node.parent), None);
    assert!(
        nodes.iter().skip(1).all(|node| node.parent == Some(0)),
        "every part hangs below the ship"
    );
    assert_eq!(
        nodes.get(1).map(|node| node.path.as_str()),
        Some("hardpoint_painted")
    );
    assert!(
        nodes.iter().all(
            |node| node.scale.iter().all(|s| (s.abs() - 1.0).abs() < 1e-5)
                && (node.rotation.iter().map(|q| q * q).sum::<f32>() - 1.0).abs() < 1e-4
        ),
        "unit scale and unit quaternions"
    );
    Ok(())
}

#[test]
fn repeated_gimbals_keep_their_own_nested_weapon_nodes() -> TestResult {
    let entity = "TEST_Dual_Gimbal_Ship";
    let written = write_entries(repeated_gimbal_assembly_entries(), entity, 0)?;
    let nodes = &written.manifest.nodes;
    let [ship, ..] = nodes.as_slice() else {
        return Err("ship node".into());
    };
    assert_eq!(ship.name, entity);
    assert_eq!(ship.parent, None);
    assert_eq!(
        nodes.iter().filter(|node| node.parent == Some(0)).count(),
        2
    );

    let half = std::f32::consts::FRAC_1_SQRT_2;
    let mut weapon_guids = Vec::new();
    let mut weapon_meshes = Vec::new();
    for (port, origin, rotation) in [
        (
            "hardpoint_gimbal_left",
            [0.0, 1.0, -4.0],
            [0.0, half, 0.0, half],
        ),
        (
            "hardpoint_gimbal_right",
            [3.0, 2.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ),
    ] {
        let weapon = weapon_on_gimbal(nodes, port, origin, rotation)?;
        let original = weapon.entity.as_ref().ok_or("weapon Record")?;
        assert_eq!(original.class_name, "TEST_Gun");
        weapon_guids.push(original.guid.as_str());
        weapon_meshes.push(weapon.mesh.as_deref().ok_or("weapon mesh")?);
    }
    assert_eq!(weapon_guids[0], weapon_guids[1]);
    assert_eq!(weapon_meshes[0], weapon_meshes[1]);
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node.name == "hardpoint_class_2/TEST_Gun")
            .count(),
        2
    );
    Ok(())
}

fn weapon_on_gimbal<'scene>(
    nodes: &'scene [Node],
    port: &str,
    origin: [f32; 3],
    rotation: [f32; 4],
) -> Result<&'scene Node, Box<dyn std::error::Error>> {
    let close = |actual: &[f32], expected: &[f32]| {
        actual.len() == expected.len()
            && actual
                .iter()
                .zip(expected)
                .all(|(a, e)| (a - e).abs() < 1e-5)
    };
    let (gimbal_index, gimbal) = nodes
        .iter()
        .enumerate()
        .find(|(_, node)| node.path == port)
        .ok_or("gimbal node")?;
    assert_eq!(gimbal.parent, Some(0), "{port} is installed on the ship");
    assert_eq!(
        gimbal.name,
        format!("{port}/TEST_Mount"),
        "the gimbal uses the port and shared Entity name"
    );
    assert!(close(&gimbal.translation, &origin), "{gimbal:?}");
    assert!(close(&gimbal.rotation, &rotation), "{gimbal:?}");
    assert!(close(&gimbal.scale, &[1.0; 3]), "{gimbal:?}");

    let mut children = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.parent == Some(gimbal_index));
    let (weapon_index, weapon) = children.next().ok_or("each gimbal must own a weapon")?;
    assert!(
        children.next().is_none(),
        "each gimbal must own only one weapon"
    );
    assert!(weapon_index > gimbal_index, "the weapon follows its parent");
    assert_eq!(
        weapon.name, "hardpoint_class_2/TEST_Gun",
        "both weapons use the same local port and Entity name"
    );
    assert_eq!(
        weapon.path,
        format!("{port}/hardpoint_class_2"),
        "the full path names the distinct parent port"
    );
    assert!(close(&weapon.translation, &[0.0, 0.5, 0.0]), "{weapon:?}");
    Ok(weapon)
}
