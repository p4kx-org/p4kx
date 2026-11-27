// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A rigid prop on an item port of a skin hangs on the bone its port names, in the skeleton of the
//! root; what is skinned stays in the bind pose of its parent.

use crate::assembly_support::{TestResult, assemble, fixture};

use p4kx_assembly::{Assembly, IssueKind, Part, Placement, PortHelperProblem, PortPlacementField};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    GARMENT_ON_SKIN_PORT, PROP_ON_BONE_PORT, PROP_ON_LOST_PORT, PROP_ON_SKIN_PORT,
    PROP_RIG_CREATURE, PROP_RIG_HUMAN, PROP_RIG_STATIC, SKINNED_ON_SKIN_PORT, prop_rig_entries,
    prop_rig_unresolved_helper_entries,
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
        .map(Part::placement)
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
fn a_skin_port_reports_helper_selection_only_when_its_rigid_anchor_is_used() -> TestResult {
    let fixture = fixture(prop_rig_unresolved_helper_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let assembly = assemble(&archive, PROP_RIG_HUMAN, 0)?;
    let baseline = build(PROP_RIG_HUMAN)?;
    let rigid_path = format!("{BODY}/{PROP_ON_SKIN_PORT}");
    let weighted_path = format!("{BODY}/{SKINNED_ON_SKIN_PORT}");
    let rigid = assembly
        .part_at(&rigid_path)
        .ok_or("the rigid attachment")?;
    let weighted = assembly
        .part_at(&weighted_path)
        .ok_or("the weighted attachment")?;
    for (part, path, expected) in [
        (rigid, rigid_path.as_str(), pelvis()),
        (weighted, weighted_path.as_str(), Placement::Skin),
    ] {
        let original = baseline
            .part_at(path)
            .ok_or("the corresponding ordinary port")?;
        assert_eq!(part.placement(), &expected, "{path}: original placement");
        assert_eq!(
            part.parent(),
            original.parent(),
            "{path}: original hierarchy"
        );
        assert_eq!(
            part.local().rows,
            original.local().rows,
            "{path}: original local pose"
        );
        assert_eq!(
            part.world().rows,
            original.world().rows,
            "{path}: original world pose"
        );
    }

    verify_helper_diagnostic(&assembly, &rigid_path, rigid, weighted)
}

fn verify_helper_diagnostic(
    assembly: &Assembly,
    rigid_path: &str,
    rigid: &Part,
    weighted: &Part,
) -> TestResult {
    let mut diagnostics = assembly
        .issues()
        .iter()
        .filter_map(|issue| match issue.kind() {
            IssueKind::PortHelperUnresolved(diagnostic) => Some((issue.path(), diagnostic)),
            _ => None,
        });
    let (path, diagnostic) = diagnostics
        .next()
        .ok_or("the applied rigid helper's diagnostic")?;
    assert_eq!(
        path, rigid_path,
        "only the actual rigid anchor uses a helper"
    );
    let additional: Vec<_> = diagnostics.collect();
    assert!(
        additional.is_empty(),
        "only the anchored port has a helper diagnostic: {additional:?}"
    );
    let placement = rigid
        .port_placement_source()
        .ok_or("the applied rigid helper fields")?;
    assert_eq!(
        diagnostic.port(),
        placement.port(),
        "the same original physical port"
    );
    assert!(
        diagnostic.attached_record_index().is_some(),
        "the diagnostic has a physical attachment Record"
    );
    assert_eq!(
        diagnostic.attached_record_index(),
        rigid.entity_record_index(),
        "the same actual attached entity"
    );
    assert_eq!(
        diagnostic.helper_ordinals(),
        [1, 0],
        "the exact authored alternative"
    );
    assert_eq!(
        diagnostic.problem(),
        PortHelperProblem::UnsupportedSelector,
        "the helper selector remains unresolved"
    );
    let weighted_source = weighted
        .port_placement_source()
        .ok_or("the bind-pose selector")?;
    assert_eq!(
        weighted_source.fields().collect::<Vec<_>>(),
        [PortPlacementField::AttachmentType],
        "the weighted attachment uses only the skin declaration"
    );
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
