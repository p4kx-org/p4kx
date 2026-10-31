// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Binding what follows the head to its joints: every level of detail, only the bones a vertex
//! uses, and the cases that are reported.

use std::sync::Arc;

use p4kx_model::Model;

use super::{bind, plan};
use crate::IssueKind;
use crate::armour::follow_blend::{HeadSkin, Unbound};
use crate::armour::head_move::{HeadMove, followed};
use crate::armour::test_models::{
    HEAD, TestResult, VERTICES, bone, hair, head_with_jaw, read, skin,
};

fn names(bones: &[p4kx_model::Bone]) -> Vec<&str> {
    bones.iter().map(|bone| bone.name.as_str()).collect()
}

#[test]
fn every_level_of_detail_is_bound_to_the_bones_its_vertices_use() -> TestResult {
    let head = head_with_jaw()?;
    let mesh = head
        .lods
        .first()
        .and_then(|lod| lod.meshes.first())
        .ok_or("a head mesh")?;
    let mut hair = hair()?;
    assert_eq!(hair.lods.len(), 2, "the hair has two levels");

    bind(
        &mut hair,
        &HeadSkin::of(&head, mesh).map_err(|error| error.to_string())?,
    )?;

    for lod in &hair.lods {
        let bones = hair.bones_of(lod);
        assert_eq!(
            names(bones),
            ["pelvis", "spine", "head", "jaw"],
            "level {}",
            lod.level
        );
        let influences = lod
            .meshes
            .first()
            .and_then(|mesh| mesh.influences.as_ref())
            .ok_or("weights")?;
        assert_eq!(influences.len(), VERTICES);
        for each in influences {
            assert_eq!(each.joints.first(), Some(&3), "level {}", lod.level);
            assert_eq!(each.sum(), 255);
        }
    }
    Ok(())
}

#[test]
fn the_parents_of_a_used_bone_join_it_and_the_unused_children_do_not() -> TestResult {
    let extra = vec![bone("neck_mid", HEAD), bone("jaw", 3), bone("tooth", 4)];
    let head = read(&skin(extra, Some(4)), "head")?;
    let mesh = head
        .lods
        .first()
        .and_then(|lod| lod.meshes.first())
        .ok_or("a head mesh")?;
    let mut hair = hair()?;

    bind(
        &mut hair,
        &HeadSkin::of(&head, mesh).map_err(|error| error.to_string())?,
    )?;

    let lod = hair.lods.first().ok_or("a level")?;
    let bones = hair.bones_of(lod);
    assert_eq!(names(bones), ["pelvis", "spine", "head", "neck_mid", "jaw"]);
    assert_eq!(bones.get(4).and_then(|jaw| jaw.parent), Some(3));
    let influences = lod
        .meshes
        .first()
        .and_then(|mesh| mesh.influences.as_ref())
        .ok_or("weights")?;
    assert!(
        influences
            .iter()
            .all(|each| each.joints.first() == Some(&4))
    );
    Ok(())
}

#[test]
fn a_head_without_weights_is_not_bound() -> TestResult {
    let head = read(&skin(vec![bone("jaw", HEAD)], None), "head")?;
    let mesh = head
        .lods
        .first()
        .and_then(|lod| lod.meshes.first())
        .ok_or("a head mesh")?;

    assert_eq!(
        HeadSkin::of(&head, mesh).err(),
        Some(Unbound::HeadNotSkinned)
    );
    Ok(())
}

#[test]
fn more_bones_than_joint_indices_are_refused() -> TestResult {
    let head = head_with_jaw()?;
    let template = head.bones.first().ok_or("a head bone")?;
    let many: Vec<_> = (0..70_000)
        .map(|index| {
            let mut bone = template.clone();
            bone.name = format!("extra_{index}");
            bone.parent = None;
            bone
        })
        .collect();

    let refused = plan(&head.bones, &many).err();

    assert_eq!(
        refused,
        Some(Unbound::TooManyBones {
            count: head.bones.len() + 70_000
        })
    );
    Ok(())
}

fn move_of(head: &Arc<Model>) -> HeadMove {
    HeadMove {
        before: Arc::clone(head),
        after: Arc::clone(head),
        morphs: Vec::new(),
    }
}

#[test]
fn a_head_that_cannot_be_followed_in_its_joints_is_reported_and_the_hair_keeps_its_bones()
-> TestResult {
    let head = Arc::new(read(&skin(vec![bone("jaw", HEAD)], None), "head")?);
    let hair = Arc::new(hair()?);

    let worn = followed(&hair, &move_of(&head));

    assert!(matches!(
        worn.kinds.as_slice(),
        [IssueKind::FollowJointsSkipped { reason }] if reason == "the head is not skinned"
    ));
    let lod = worn.model.lods.first().ok_or("a level")?;
    let influences = lod
        .meshes
        .first()
        .and_then(|mesh| mesh.influences.as_ref())
        .ok_or("weights")?;
    assert!(
        influences
            .iter()
            .all(|each| each.joints.first() == Some(&HEAD))
    );
    assert_eq!(worn.model.bones.len(), 3);
    Ok(())
}

#[test]
fn a_skinned_head_binds_the_hair_without_a_report() -> TestResult {
    let head = Arc::new(head_with_jaw()?);
    let hair = Arc::new(hair()?);

    let worn = followed(&hair, &move_of(&head));

    assert!(worn.kinds.is_empty(), "{:?}", worn.kinds);
    let lod = worn.model.lods.first().ok_or("a level")?;
    assert_eq!(worn.model.bones_of(lod).len(), 4);
    Ok(())
}
