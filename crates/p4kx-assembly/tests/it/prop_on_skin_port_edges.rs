// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The edges of the props on skin ports: the human root stays unbound, a helper that only the skin
//! holds does not move a prop, and a root without a readable skeleton anchors nothing.

use crate::assembly_support::{TestResult, assemble, fixture};

use p4kx_assembly::{Assembly, Placement};
use p4kx_extraction::Archive;
use p4kx_gltf::GlbOptions;
use p4kx_testkit::{
    EMPTY_ON_SKIN_PORT, NODE_ON_SKIN_PORT, PROP_ON_BONE_PORT, PROP_ON_SKIN_PORT,
    PROP_RIG_BAD_SKELETON, PROP_RIG_HUMAN, PROP_RIG_NO_SKELETON, check_glb, prop_rig_entries,
};

const BODY: &str = "Body_ItemPort";

fn build(class: &str) -> Result<Assembly, Box<dyn std::error::Error>> {
    let fixture = fixture(prop_rig_entries())?;
    let archive = Archive::open(&fixture.path)?;
    assemble(&archive, class, 0)
}

fn placement<'a>(assembly: &'a Assembly, port: &str) -> Result<&'a Placement, &'static str> {
    assembly
        .part_at(&format!("{BODY}/{port}"))
        .map(p4kx_assembly::Part::placement)
        .ok_or("the part")
}

#[test]
fn a_human_root_writes_no_joints_whatever_its_props_anchor_to() -> TestResult {
    let assembly = build(PROP_RIG_HUMAN)?;

    let mut bytes = Vec::new();
    let summary = assembly.write_glb(&GlbOptions::default(), &mut bytes)?;
    let glb = check_glb(&bytes)?;
    assert_eq!(glb.document.skins().len(), 0);
    assert_eq!((summary.bones, summary.skinned_meshes), (0, 0));
    Ok(())
}

#[test]
fn a_helper_only_the_skin_holds_as_a_node_does_not_move_a_prop() -> TestResult {
    let assembly = build(PROP_RIG_HUMAN)?;

    assert_eq!(placement(&assembly, NODE_ON_SKIN_PORT)?, &Placement::Skin);
    assert!(
        assembly
            .issues()
            .iter()
            .all(|issue| issue.kind().code() != "helper_not_found"),
        "{:?}",
        assembly.issues()
    );
    Ok(())
}

#[test]
fn a_prop_without_meshes_hangs_on_its_bone_like_any_unskinned_model() -> TestResult {
    let assembly = build(PROP_RIG_HUMAN)?;

    let part = assembly
        .part_at(&format!("{BODY}/{EMPTY_ON_SKIN_PORT}"))
        .ok_or("the prop")?;
    assert_eq!(part.placement(), &Placement::Helper("pelvis".to_owned()));
    Ok(())
}

#[test]
fn a_root_without_a_readable_skeleton_leaves_props_in_the_bind_pose() -> TestResult {
    for class in [PROP_RIG_NO_SKELETON, PROP_RIG_BAD_SKELETON] {
        let assembly = build(class)?;

        assert_eq!(
            placement(&assembly, PROP_ON_SKIN_PORT)?,
            &Placement::Skin,
            "{class}"
        );
        assert_eq!(
            placement(&assembly, PROP_ON_BONE_PORT)?,
            &Placement::Origin,
            "{class}"
        );
        let listed: Vec<&str> = assembly
            .issues()
            .iter()
            .filter(|issue| issue.kind().code() == "helper_not_found")
            .map(p4kx_assembly::Issue::path)
            .collect();
        assert_eq!(listed, [format!("{BODY}/{PROP_ON_BONE_PORT}")], "{class}");
    }
    Ok(())
}
