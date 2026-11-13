// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::*;
use crate::ContainerSource;

fn reference(position: [f32; 3]) -> ObjectContainerRef {
    ObjectContainerRef {
        file: "a.socpak".to_owned(),
        bone: String::new(),
        position,
        rotation: [0.0; 3],
        source: ContainerSource::Vehicle,
    }
}

#[test]
fn a_non_finite_offset_is_the_identity_and_an_issue() {
    let mut built = EntityInterior::default();

    let placed = built.place(INTERIOR_NODE, &reference([f32::NAN, 0.0, 0.0]), None);

    assert!(placed.is_identity());
    assert_eq!(built.issues.len(), 1);
    assert_eq!(built.issues[0].kind.code(), "invalid_offset");
}

#[test]
fn an_assembly_without_parts_still_writes_the_interior_at_the_top() {
    let empty = Assembly {
        forge_origin: None,
        parts: Vec::new(),
        issues: Vec::new(),
        lod: 0,
        interior: None,
        default_material: None,
        containers: Vec::new(),
    };
    let mut interior = EntityInterior::default();
    interior.add(
        0,
        "",
        &reference([0.0; 3]),
        Transform::IDENTITY,
        Interior {
            parts: Vec::new(),
            issues: Vec::new(),
            lod: 0,
            containers: 0,
            default_material: None,
        },
    );

    let assembly = empty.with_interior(interior);
    let mut scene = Scene::new();
    let written = assembly
        .interior()
        .map(|interior| interior.add_to_scene(&mut scene, 0));

    assert!(matches!(written, Some(Ok(()))));
    let [group] = scene.nodes() else {
        panic!("{} nodes", scene.nodes().len());
    };
    assert_eq!(group.name, INTERIOR_NODE);
    assert_eq!(group.parent, None);
}
