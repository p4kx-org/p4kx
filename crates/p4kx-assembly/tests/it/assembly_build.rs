// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Assembly of the synthetic ship: parts, placement, material libraries and issues.

use crate::assembly_support;

use assembly_support::{TestResult, assemble, entries, fixture};
use p4kx_assembly::{IssueKind, Part, Placement, PortPlacementField};
use p4kx_extraction::model::{MaterialStatus, ModelFile};
use p4kx_extraction::{Archive, SourceEncryption};
use p4kx_testkit::{ASSEMBLY_GUN_PAINT as GUN_PAINT, test_key};

fn close(actual: [f32; 4], expected: [f32; 4]) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(a, e)| (a - e).abs() < 1e-5)
}

#[test]
fn the_parts_follow_the_loadout_tree_and_are_named_after_port_and_entity() -> TestResult {
    let fixture = fixture(entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, "TEST_Ship", 0)?;

    let names: Vec<&str> = assembly.parts().iter().map(Part::name).collect();
    assert_eq!(
        names,
        [
            "TEST_Ship",
            "hardpoint_weapon_nose/TEST_Mount",
            "hardpoint_class_2/TEST_Gun",
            "hardpoint_relay/TEST_Relay",
            "hardpoint_seat/TEST_Seat",
            "hardpoint_door/TEST_Door",
        ]
    );
    let gun = assembly
        .part_at("hardpoint_weapon_nose/hardpoint_class_2")
        .ok_or("the gun")?;
    let mount = assembly
        .part_at("hardpoint_weapon_nose")
        .ok_or("the mount")?;
    assert_eq!(gun.parent(), Some(1), "the gun hangs below the mount");
    assert_eq!(mount.parent(), Some(0), "the mount hangs below the ship");
    assert_eq!(assembly.root().ok_or("a root")?.parent(), None);
    Ok(())
}

#[test]
fn world_transforms_come_from_the_helpers_and_port_offsets() -> TestResult {
    let fixture = fixture(entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, "TEST_Ship", 0)?;

    let mount = assembly
        .part_at("hardpoint_weapon_nose")
        .ok_or("the mount")?;
    assert_eq!(mount.placement(), &Placement::Helper("hp_nose".to_owned()));
    let placement = mount
        .port_placement_source()
        .ok_or("the original helper inputs")?;
    assert_eq!(placement.port().port_ordinal(), 0);
    assert_eq!(
        placement.fields().collect::<Vec<_>>(),
        [
            PortPlacementField::Helper,
            PortPlacementField::Position,
            PortPlacementField::Rotation,
        ]
    );
    assert!(
        close(mount.local().rows[0], [0.0, -1.0, 0.0, 0.0])
            && close(mount.local().rows[1], [1.0, 0.0, 0.0, 4.0])
            && close(mount.local().rows[2], [0.0, 0.0, 1.0, 1.0]),
        "helper at (0, 4, 0), offset (0, 0, 1) turned 90 degrees about Z: {:?}",
        mount.local()
    );
    let gun = assembly
        .part_at("hardpoint_weapon_nose/hardpoint_class_2")
        .ok_or("the gun")?;
    assert_eq!(
        gun.world().translation(),
        [0.0, 4.0, 1.5],
        "the gun's helper 0.5 m above the mount, which stands 1 m up"
    );
    assert!(
        close(gun.world().rows[1], [1.0, 0.0, 0.0, 4.0]),
        "the mount's turn carries over: {:?}",
        gun.world()
    );
    let relay = assembly.part_at("hardpoint_relay").ok_or("the relay")?;
    assert_eq!(
        relay.placement(),
        &Placement::PortNode,
        "no helper, so the node named like the port"
    );
    assert_eq!(relay.world().translation(), [3.0, 0.0, 0.0]);
    assert!(
        relay.port_placement_source().is_none(),
        "the port name comes from the loadout"
    );
    Ok(())
}

#[test]
fn unresolved_parts_are_skipped_and_listed() -> TestResult {
    let fixture = fixture(entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, "TEST_Ship", 0)?;

    let found: Vec<(&str, &str)> = assembly
        .issues()
        .iter()
        .map(|issue| (issue.path(), issue.kind().code()))
        .collect();
    assert_eq!(
        found,
        [
            ("hardpoint_cargo", "unresolved"),
            ("hardpoint_seat", "no_geometry"),
            ("hardpoint_door", "geometry_missing"),
        ]
    );
    assert!(
        assembly
            .issues()
            .iter()
            .all(|issue| issue.kind().skips() || issue.kind().code() == "no_geometry"),
    );
    let door = assembly
        .part_at("hardpoint_door")
        .ok_or("the door keeps its node")?;
    assert!(door.model().is_none());
    assert_eq!(
        assembly.modelled_parts(),
        4,
        "ship, mount, gun and relay have models"
    );
    assert!(
        assembly.part_at("hardpoint_cargo").is_none(),
        "an unresolved Entity is no part"
    );
    Ok(())
}

#[test]
fn the_material_library_of_the_entity_replaces_the_one_of_the_model() -> TestResult {
    let fixture = fixture(entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, "TEST_Ship", 0)?;

    let gun = assembly
        .part_at("hardpoint_weapon_nose/hardpoint_class_2")
        .ok_or("the gun")?;
    assert_eq!(
        gun.model().ok_or("a model")?.material_library.as_deref(),
        Some("objects/test/cube"),
        "the model names the blue library"
    );
    let library = gun.materials().library().ok_or("a library")?;
    assert_eq!(library.path, GUN_PAINT, "the Entity's library is drawn");
    let [red, green, blue] = library.materials.get(1).ok_or("a material")?.diffuse;
    assert!(
        close([red, green, blue, 1.0], [0.1, 0.8, 0.1, 1.0]),
        "green, not the model's blue"
    );
    let ship = assembly.root().ok_or("a root")?;
    assert_eq!(
        ship.materials()
            .library()
            .map(|library| library.path.as_str()),
        Some("objects/test/ship"),
        "without an override the model's own library applies"
    );
    Ok(())
}

#[test]
fn libraries_and_models_are_shared_among_parts() -> TestResult {
    let fixture = fixture(entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, "TEST_Ship", 0)?;

    let same = |a: &str, b: &str| -> Result<bool, &'static str> {
        let a = assembly.part_at(a).ok_or("part a")?;
        let b = assembly.part_at(b).ok_or("part b")?;
        Ok(std::ptr::eq(
            a.model().ok_or("model a")?,
            b.model().ok_or("model b")?,
        ))
    };
    assert!(
        same("hardpoint_weapon_nose/hardpoint_class_2", "hardpoint_relay")?,
        "the gun and the relay use one model read once"
    );
    assert!(
        matches!(
            assembly.root().ok_or("root")?.materials(),
            MaterialStatus::Loaded(_)
        ),
        "loaded"
    );
    Ok(())
}

#[test]
fn the_level_of_detail_applies_to_every_model_that_has_it() -> TestResult {
    let fixture = fixture(entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, "TEST_Ship", 1)?;

    let level = |path: &str| -> Result<u32, &'static str> {
        let part = assembly.part_at(path).ok_or("a part")?;
        Ok(part.model().ok_or("a model")?.lods[0].level)
    };
    assert_eq!(level("hardpoint_weapon_nose")?, 0, "the mount has no LOD1");
    assert_eq!(assembly.lod(), 1);
    let ship = assembly.root().ok_or("root")?.model().ok_or("model")?;
    assert_eq!(ship.lods[0].level, 1, "the ship has LOD1");
    Ok(())
}

#[test]
fn a_level_beyond_every_model_falls_back_to_the_deepest_one_each_has() -> TestResult {
    let fixture = fixture(entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, "TEST_Ship", 15)?;

    let level = |path: &str| -> Result<(usize, u32), &'static str> {
        let model = assembly
            .part_at(path)
            .ok_or("a part")?
            .model()
            .ok_or("a model")?;
        Ok((model.lods.len(), model.lods[0].level))
    };
    assert_eq!(
        level("hardpoint_weapon_nose")?,
        (1, 0),
        "the mount has only LOD0"
    );
    let ship = assembly.root().ok_or("root")?.model().ok_or("model")?;
    assert_eq!(
        (ship.lods.len(), ship.lods[0].level),
        (1, 1),
        "the ship keeps just LOD1"
    );
    Ok(())
}
#[test]
fn a_missing_library_is_reported_once_and_the_part_keeps_its_model() -> TestResult {
    let kept: Vec<_> = entries()
        .into_iter()
        .filter(|entry| !format!("{entry:?}").contains("gun_paint.mtl"))
        .collect();
    let fixture = fixture(kept)?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive, "TEST_Ship", 0)?;

    let problems: Vec<_> = assembly
        .issues()
        .iter()
        .filter(|issue| matches!(issue.kind(), IssueKind::MaterialLibrary { .. }))
        .collect();
    assert_eq!(
        problems.len(),
        1,
        "gun and relay share the library: {problems:?}"
    );
    let gun = assembly
        .part_at("hardpoint_weapon_nose/hardpoint_class_2")
        .ok_or("the gun")?;
    assert!(gun.model().is_some(), "still drawn");
    assert!(
        gun.materials().library().is_none(),
        "the model's own library does not take over"
    );
    Ok(())
}

#[test]
fn placed_models_and_libraries_keep_only_their_selected_archive_reads() -> TestResult {
    let fixture = fixture(
        entries()
            .into_iter()
            .map(|entry| {
                if entry.path().ends_with("ship_lod1.cgfm") || entry.path().ends_with("ship.mtl") {
                    entry.encrypted()
                } else {
                    entry
                }
            })
            .collect(),
    )?;
    let archive = Archive::open_with_key(&fixture.path, Some(test_key()))?;
    let detailed = assemble(&archive, "TEST_Ship", 0)?;
    let coarse = assemble(&archive, "TEST_Ship", 1)?;
    for (assembly, role, path, encrypted) in [
        (
            &detailed,
            ModelFile::Geometry,
            "ship.cgfm",
            SourceEncryption::Unencrypted,
        ),
        (
            &coarse,
            ModelFile::LodGeometry(1),
            "ship_lod1.cgfm",
            SourceEncryption::Encrypted,
        ),
    ] {
        let root = assembly.root().ok_or("placed ship")?;
        let geometry = root
            .model_archive_sources()
            .iter()
            .find(|read| read.file() == role)
            .and_then(|read| read.origin())
            .ok_or("selected model input")?;
        assert!(geometry.entry().path().ends_with(path));
        assert_eq!(geometry.encryption(), encrypted);
        let excluded = if role == ModelFile::Geometry {
            ModelFile::LodGeometry(1)
        } else {
            ModelFile::Geometry
        };
        assert!(
            root.model_archive_sources()
                .iter()
                .all(|read| read.file() != excluded)
        );
        let library = root
            .material_archive_origins()
            .library()
            .ok_or("used ship material library")?;
        assert!(library.entry().path().ends_with("ship.mtl"));
        assert_eq!(library.encryption(), SourceEncryption::Encrypted);
        without_model_inputs(assembly)?;
    }
    Ok(())
}

fn without_model_inputs(assembly: &p4kx_assembly::Assembly) -> TestResult {
    let seat = assembly
        .part_at("hardpoint_seat")
        .ok_or("seat without geometry")?;
    assert!(
        seat.model_archive_sources().is_empty(),
        "the seat has no model read"
    );
    assert!(
        seat.material_archive_origins().library().is_none(),
        "the seat has no library read"
    );
    Ok(())
}
