// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The hair follows the head's facial joints: it is bound to the joints its head vertices are
//! bound to, and the joints join its skeleton.

use std::error::Error;

use p4kx_assembly::{
    ArchiveModels, Armour, ArmourPart, AssemblyOptions, BodyVariant, Expressions, Piece,
};
use p4kx_chunks::ChunkFile;
use p4kx_extraction::Archive;
use p4kx_gltf::GlbOptions;
use p4kx_model::{Animation, BoneMatch, Influences, MAX_INFLUENCES};
use p4kx_testkit::{
    DRESSED_JAW_BONE, DRESSED_JAW_INDEX, HEAD_VERTICES, MALE_RIG_HASH, TestAnimation, TestSkeleton,
    TestTrack, check_glb, dressed_entries, dressed_jaw_entries, head_rig,
};

use crate::assembly_support::{TestResult, fixture};
use crate::head_build::{dna_of_head_two_in_region_three, head_path};

const HAIR: &str = "Objects/Characters/Human/heads/shared/hair_v2/sc/male/m_hair_test.skin";

fn built(jaw: bool) -> Result<(Armour, Archive, crate::assembly_support::Fixture), Box<dyn Error>> {
    let rig = Some(head_rig(HEAD_VERTICES, MALE_RIG_HASH).build());
    let fixture = fixture(if jaw {
        dressed_jaw_entries(rig)
    } else {
        dressed_entries(rig)
    })?;
    let archive = Archive::open(&fixture.path)?;
    let head = Piece::file("head", head_path(), None)
        .shaped(dna_of_head_two_in_region_three())
        .with_expressions(Expressions::All);
    let hair = Piece::file("hair", HAIR, None).following_head();
    let armour = Armour::build(
        BodyVariant::Male,
        &[head, hair],
        &ArchiveModels::new(&archive),
        &AssemblyOptions::default(),
    )?;
    Ok((armour, archive, fixture))
}

fn hair(armour: &Armour) -> Result<&ArmourPart, Box<dyn Error>> {
    Ok(armour
        .parts()
        .iter()
        .find(|part| part.name() == "hair")
        .ok_or("no hair part")?)
}

/// Returns the bones of the skeleton of `armour` that the vertices of the hair are weighted to.
fn skeleton_influences(armour: &Armour) -> Result<Vec<Influences>, Box<dyn Error>> {
    let part = hair(armour)?;
    let lod = part.model().lods.first().ok_or("no level")?;
    let matched = BoneMatch::new(&armour.skeleton().bones, part.model().bones_of(lod));
    Ok(lod
        .meshes
        .first()
        .and_then(|mesh| mesh.influences.as_ref())
        .ok_or("the hair is not skinned")?
        .iter()
        .map(|influences| influences.remapped(|joint| matched.target(joint)))
        .collect())
}

#[test]
fn the_hair_is_bound_to_the_facial_joint_its_head_vertices_are_bound_to() -> TestResult {
    let (armour, _archive, _dir) = built(true)?;

    let jaw = armour
        .skeleton()
        .bones
        .iter()
        .position(|bone| bone.name == DRESSED_JAW_BONE)
        .ok_or("the jaw joins the skeleton")?;
    let influences = skeleton_influences(&armour)?;

    assert_eq!(influences.len(), HEAD_VERTICES);
    for each in &influences {
        assert_eq!(usize::from(*each.joints.first().ok_or("a joint")?), jaw);
        assert!(each.weights.first().copied() > Some(200), "{each:?}");
        assert_eq!(each.sum(), 255);
        assert!(each.count() <= MAX_INFLUENCES);
    }
    assert!(armour.issues().is_empty(), "{:?}", armour.issues());
    Ok(())
}

#[test]
fn a_head_without_facial_joints_leaves_the_hair_on_the_head_bone() -> TestResult {
    let (armour, _archive, _dir) = built(false)?;

    let head = armour
        .skeleton()
        .bones
        .iter()
        .position(|bone| bone.name == "head")
        .ok_or("the head bone")?;
    for each in skeleton_influences(&armour)? {
        assert_eq!(usize::from(*each.joints.first().ok_or("a joint")?), head);
        assert_eq!(each.count(), 1, "{each:?}");
    }
    Ok(())
}

#[test]
fn the_binding_is_the_same_every_time() -> TestResult {
    let (first, _a, _da) = built(true)?;
    let (second, _b, _db) = built(true)?;

    assert_eq!(skeleton_influences(&first)?, skeleton_influences(&second)?);
    Ok(())
}

fn jaw_animation() -> Result<Animation, Box<dyn Error>> {
    let source = TestAnimation::new("open.caf", 30).track(
        TestTrack::new(TestSkeleton::controller_id(usize::from(DRESSED_JAW_INDEX)))
            .rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (10, [0.2, 0.0, 0.0, 0.98])]),
    );
    let bytes = source.caf();
    Ok(Animation::read_caf(
        source.name(),
        &ChunkFile::parse(bytes.as_slice())?,
    )?)
}

#[test]
fn a_jaw_animation_is_written_with_the_hair_bound_to_the_jaw() -> TestResult {
    let (armour, archive, _dir) = built(true)?;
    let models = ArchiveModels::new(&archive);
    let list = [jaw_animation()?];

    let animations = armour.animate(&list, &models);

    assert!(animations.issues().is_empty(), "{:?}", animations.issues());
    let mut bytes = Vec::new();
    let summary = armour.write_glb_animated(&animations, &GlbOptions::default(), &mut bytes)?;
    check_glb(&bytes)?;
    assert_eq!(summary.animations.len(), 1);
    Ok(())
}
