// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A rigid prop on an item port of a skin hangs on the bone its port names, in the skeleton of the
//! root; what is skinned stays in the bind pose of its parent.

use crate::assembly_support::{TestResult, assemble, fixture};

use p4kx_assembly::{Assembly, Placement};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    GARMENT_ON_SKIN_PORT, PROP_ON_BONE_PORT, PROP_ON_LOST_PORT, PROP_ON_SKIN_PORT,
    PROP_RIG_CREATURE, PROP_RIG_HUMAN, PROP_RIG_STATIC, SKINNED_ON_SKIN_PORT, prop_rig_entries,
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

fn pelvis() -> Placement {
    Placement::Helper("pelvis".to_owned())
}

#[test]
fn a_rigid_prop_on_a_skin_port_hangs_on_the_bone_of_a_human_skeleton() -> TestResult {
    let assembly = build(PROP_RIG_HUMAN)?;

    assert_eq!(placement(&assembly, PROP_ON_SKIN_PORT)?, &pelvis());
    let watch = assembly
        .part_at(&format!("{BODY}/{PROP_ON_SKIN_PORT}"))
        .ok_or("the prop")?;
    assert_eq!(watch.world().translation(), [0.0, 0.0, 1.0]);
    assert!(assembly.issues().iter().all(|issue| {
        !matches!(
            issue.kind().code(),
            "helper_not_found" | "bones_not_in_skeleton"
        )
    }));
    Ok(())
}

#[test]
fn a_rigid_prop_on_a_skin_port_hangs_on_the_bone_of_a_creature_skeleton() -> TestResult {
    let assembly = build(PROP_RIG_CREATURE)?;

    assert_eq!(placement(&assembly, PROP_ON_SKIN_PORT)?, &pelvis());
    Ok(())
}

#[test]
fn a_bone_the_skeleton_lacks_leaves_the_prop_in_the_bind_pose_without_a_note() -> TestResult {
    let assembly = build(PROP_RIG_HUMAN)?;

    assert_eq!(placement(&assembly, PROP_ON_LOST_PORT)?, &Placement::Skin);
    assert!(
        assembly
            .issues()
            .iter()
            .all(|issue| issue.path() != format!("{BODY}/{PROP_ON_LOST_PORT}"))
    );
    Ok(())
}

#[test]
fn a_skinned_part_on_a_skin_port_stays_in_the_bind_pose() -> TestResult {
    let assembly = build(PROP_RIG_HUMAN)?;

    assert_eq!(
        placement(&assembly, SKINNED_ON_SKIN_PORT)?,
        &Placement::Skin
    );
    Ok(())
}

#[test]
fn a_rigid_character_item_on_a_skin_port_stays_in_the_bind_pose() -> TestResult {
    let assembly = build(PROP_RIG_HUMAN)?;

    assert_eq!(
        placement(&assembly, GARMENT_ON_SKIN_PORT)?,
        &Placement::Skin
    );
    Ok(())
}

#[test]
fn a_bone_port_finds_its_bone_in_the_skeleton_of_the_human_root() -> TestResult {
    let assembly = build(PROP_RIG_HUMAN)?;

    assert_eq!(placement(&assembly, PROP_ON_BONE_PORT)?, &pelvis());
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
fn a_root_that_is_no_character_file_has_no_skeleton_to_hang_props_on() -> TestResult {
    let assembly = build(PROP_RIG_STATIC)?;

    assert_eq!(placement(&assembly, PROP_ON_SKIN_PORT)?, &Placement::Skin);
    assert_eq!(placement(&assembly, PROP_ON_BONE_PORT)?, &Placement::Origin);
    Ok(())
}
