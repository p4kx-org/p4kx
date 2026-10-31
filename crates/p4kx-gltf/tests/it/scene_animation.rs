// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A scene given a skeleton and animations writes the animations on its bone nodes.

use p4kx_chunks::ChunkFile;
use p4kx_gltf::{GlbOptions, GlbSummary, Scene, SceneNode, ScenePart};
use p4kx_model::{Animation, MaterialStatus, Model, Transform};
use p4kx_testkit::{CheckedGlb, TestAnimation, TestModel, TestSkeleton, TestTrack, check_glb};

use crate::rig::{body, piece_model, skeleton_model};
use crate::scene_support::{TestResult, read_cube, translation};

const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const QUARTER_TURN_Z: [f32; 4] = [
    0.0,
    0.0,
    std::f32::consts::FRAC_1_SQRT_2,
    std::f32::consts::FRAC_1_SQRT_2,
];

fn animation(source: &TestAnimation) -> Result<Animation, Box<dyn std::error::Error>> {
    Ok(Animation::read_caf(
        source.name(),
        &ChunkFile::parse(source.caf())?,
    )?)
}

fn nod(bone: usize) -> TestAnimation {
    TestAnimation::new("nod.caf", 30).track(
        TestTrack::new(TestSkeleton::controller_id(bone))
            .rotations(&[(0, IDENTITY), (30, QUARTER_TURN_Z)]),
    )
}

fn write(
    skeleton: &Model,
    parts: &[(&str, &Model)],
    animations: &[Animation],
) -> Result<(CheckedGlb, GlbSummary), Box<dyn std::error::Error>> {
    let materials = MaterialStatus::Unnamed;
    let mut scene = Scene::new()
        .with_skeleton(&skeleton.bones)
        .with_animations(animations);
    let root = scene.push(SceneNode::new("root", None, Transform::IDENTITY))?;
    for (name, model) in parts {
        let node = SceneNode::new(*name, Some(root), translation(0.0, 0.0, 5.0))
            .with_part(ScenePart::new(model, &materials));
        scene.push(node)?;
    }
    let mut bytes = Vec::new();
    let summary = p4kx_gltf::write_scene_glb(&scene, &GlbOptions::default(), &mut bytes)?;
    Ok((check_glb(&bytes)?, summary))
}

#[test]
fn an_animation_moves_the_bone_node_its_controller_names() -> TestResult {
    let skeleton = skeleton_model(&body())?;
    let skin = piece_model(&TestModel::two_bone_skin())?;
    let nodded = animation(&nod(1))?;

    let (glb, summary) = write(&skeleton, &[("body/skin", &skin)], &[nodded])?;

    let written = glb.document.animations().next().ok_or("an animation")?;
    assert_eq!(written.name(), Some("nod"));
    let channel = written.channels().next().ok_or("a channel")?;
    let target = channel.target().node();
    assert_eq!(target.name(), Some("TIP"));
    assert!(
        glb.document
            .skins()
            .flat_map(|skin| skin.joints().collect::<Vec<_>>())
            .any(|joint| joint.index() == target.index()),
        "the target is a joint of the skin"
    );
    let [animation] = summary.animations.as_slice() else {
        return Err("one animation summary".into());
    };
    assert_eq!((animation.channels, animation.keys), (1, 2));
    Ok(())
}

#[test]
fn a_track_without_a_bone_is_left_out_and_listed() -> TestResult {
    let skeleton = skeleton_model(&body())?;
    let skin = piece_model(&TestModel::two_bone_skin())?;
    let stray = animation(&nod(9))?;

    let (glb, summary) = write(&skeleton, &[("body/skin", &skin)], &[stray])?;

    assert_eq!(glb.document.animations().len(), 0);
    let [animation] = summary.animations.as_slice() else {
        return Err("one animation summary".into());
    };
    assert_eq!(animation.skipped_tracks, [TestSkeleton::controller_id(9)]);
    Ok(())
}

#[test]
fn a_scene_without_a_skinned_mesh_writes_no_animation() -> TestResult {
    let skeleton = skeleton_model(&body())?;
    let cube = read_cube()?;
    let nodded = animation(&nod(1))?;

    let (glb, summary) = write(&skeleton, &[("prop", &cube)], &[nodded])?;

    assert_eq!(glb.document.animations().len(), 0);
    assert_eq!(summary.animations.len(), 0);
    Ok(())
}
