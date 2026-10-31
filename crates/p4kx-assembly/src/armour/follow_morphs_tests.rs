// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The expression targets of what follows the head, at every level of detail.

use std::sync::Arc;

use p4kx_gltf::MorphTarget;
use p4kx_model::Model;

use super::{Followed, follow_morphs};
use crate::IssueKind;
use crate::armour::head_move::{HeadMove, first_mesh_note, followed};
use crate::armour::test_models::{TestResult, hair_at, head_at};

fn blink() -> MorphTarget {
    MorphTarget {
        name: "blink".to_owned(),
        vertices: vec![0],
        deltas: vec![[0.0, 0.0, 0.002]],
    }
}

fn follow(model: &Model, head: &Model, targets: &[MorphTarget]) -> TestResult<Followed> {
    let mesh = head
        .lods
        .first()
        .and_then(|lod| lod.meshes.first())
        .ok_or("a head mesh")?;
    Ok(follow_morphs(model, mesh, targets))
}

fn check_blink(made: &Followed, what: &str) -> TestResult {
    assert_eq!(made.of, [0], "{what}");
    let [target] = made.targets.as_slice() else {
        return Err(format!("one target: {what}").into());
    };
    assert_eq!(target.name, "blink", "{what}");
    assert!(
        target.deltas.iter().all(|by| by[2] > 0.0),
        "{what}: {:?}",
        target.deltas
    );
    Ok(())
}

fn move_of(head: &Arc<Model>) -> HeadMove {
    HeadMove {
        before: Arc::clone(head),
        after: Arc::clone(head),
        morphs: vec![blink()],
    }
}

#[test]
fn the_hair_gets_the_targets_of_the_head_of_its_own_level() -> TestResult {
    for level in 0..2 {
        let hair = hair_at(level)?;
        assert_eq!(
            hair.lods.first().map(|lod| usize::try_from(lod.level)),
            Some(Ok(level))
        );

        let made = follow(&hair, &head_at(level)?, &[blink()])?;

        check_blink(&made, &format!("level {level}"))?;
    }
    Ok(())
}

#[test]
fn a_hair_and_a_head_of_different_levels_still_pair_up_by_position() -> TestResult {
    for (hair, head) in [(0, 1), (1, 0)] {
        let made = follow(&hair_at(hair)?, &head_at(head)?, &[blink()])?;

        check_blink(&made, &format!("hair {hair}, head {head}"))?;
    }
    Ok(())
}

#[test]
fn a_head_without_targets_gives_no_targets_at_any_level() -> TestResult {
    for level in 0..2 {
        let made = follow(&hair_at(level)?, &head_at(level)?, &[])?;

        assert!(
            made.targets.is_empty() && made.of.is_empty(),
            "level {level}"
        );
    }
    Ok(())
}

#[test]
fn a_lower_level_carries_the_targets_without_a_face_link() -> TestResult {
    let head = Arc::new(head_at(1)?);
    let hair = Arc::new(hair_at(1)?);

    let worn = followed(&hair, &move_of(&head));

    assert_eq!(worn.morphs.len(), worn.follows.len());
    assert_eq!(worn.follows, [0]);
    assert!(worn.face.is_none(), "only a head has a face");
    assert!(worn.kinds.is_empty(), "{:?}", worn.kinds);
    Ok(())
}

#[test]
fn several_meshes_at_a_lower_level_are_noted_when_there_are_targets() -> TestResult {
    let head = Arc::new(head_at(1)?);
    let mut hair = hair_at(1)?;
    let extra = hair
        .lods
        .first()
        .and_then(|lod| lod.meshes.first())
        .cloned()
        .ok_or("a hair mesh")?;
    hair.lods.first_mut().ok_or("a level")?.meshes.push(extra);
    let mut worn = followed(&Arc::new(hair), &move_of(&head));

    assert_eq!(
        first_mesh_note(&worn),
        Some(IssueKind::FollowMorphsFirstMeshOnly { meshes: 2 })
    );
    worn.morphs.clear();
    assert_eq!(first_mesh_note(&worn), None, "no targets, no note");
    Ok(())
}

#[test]
fn one_mesh_at_a_lower_level_is_not_noted() -> TestResult {
    let head = Arc::new(head_at(1)?);

    let worn = followed(&Arc::new(hair_at(1)?), &move_of(&head));

    assert_eq!(first_mesh_note(&worn), None);
    Ok(())
}
