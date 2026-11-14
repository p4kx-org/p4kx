// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The face of a story character's own head: the joints of its `.RigLogic` file move with a face
//! animation, and the blendshapes of its geometry file are its expressions.

use std::error::Error;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Armour, ArmourPart, AssemblyOptions, Expressions, IssueKind,
    LoadoutResolver,
};
use p4kx_chunks::ChunkFile;
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_gltf::GlbOptions;
use p4kx_model::Animation;
use p4kx_testkit::{
    NPC_CLASS, NPC_FACIAL_BONE, NPC_SKELETON_FILE, NPC_SKELETON_SHIFT, TestAnimation, TestBone,
    TestEntry, TestSkeleton, TestSkinBlendshapes, TestTrack, check_glb, npc_entries,
    npc_face_entries, npc_face_entries_posed_by, npc_face_entries_with, npc_face_logic,
    npc_face_shapes, npc_face_skeleton,
};

use crate::assembly_support::{Fixture, TestResult, fixture};

struct Npc {
    armour: Armour,
    archive: Archive,
    _fixture: Fixture,
}

fn build(entries: Vec<TestEntry>, expressions: &Expressions) -> Result<Npc, Box<dyn Error>> {
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let armour = {
        let files = ArchiveFiles::new(&archive);
        let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
        let record = resolver.entities().find(NPC_CLASS)?;
        let tree = resolver.resolve(&record)?;
        let head = resolver
            .npc_head(&tree)
            .ok_or("the tree has no head")?
            .with_expressions(expressions);
        let mut pieces = resolver.worn(&tree);
        pieces.extend(head.pieces().iter().cloned());
        Armour::build(
            head.variant(),
            &pieces,
            &ArchiveModels::new(&archive),
            &AssemblyOptions::default(),
        )?
    };
    Ok(Npc {
        armour,
        archive,
        _fixture: fixture,
    })
}

fn head(npc: &Npc) -> Result<&ArmourPart, Box<dyn Error>> {
    Ok(npc
        .armour
        .parts()
        .iter()
        .find(|part| part.name() == "head")
        .ok_or("the head part")?)
}

fn animation(source: &TestAnimation) -> Result<Animation, Box<dyn Error>> {
    let bytes = source.caf();
    Ok(Animation::read_caf(
        source.name(),
        &ChunkFile::parse(bytes.as_slice())?,
    )?)
}

fn control(name: &str) -> Result<Animation, Box<dyn Error>> {
    animation(&TestAnimation::new("face.caf", 30).track(
        TestTrack::new(crc32fast::hash(name.as_bytes())).positions(&[
            (0, [0.0, 0.0, 0.0]),
            (10, [1.0, 0.0, 0.0]),
            (20, [0.0, 0.0, 0.0]),
        ]),
    ))
}

fn close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-5,
        "{actual} against {expected}"
    );
}

fn face_issues(npc: &Npc) -> Vec<&IssueKind> {
    npc.armour
        .issues()
        .iter()
        .map(p4kx_assembly::Issue::kind)
        .filter(|kind| {
            matches!(
                kind,
                IssueKind::FaceJointsFailed { .. } | IssueKind::MorphsFailed { .. }
            )
        })
        .collect()
}

#[test]
fn the_blendshapes_of_the_geometry_are_the_expressions_of_the_head() -> TestResult {
    let npc = build(npc_face_entries(), &Expressions::All)?;

    let part = head(&npc)?;

    let names: Vec<&str> = part
        .morphs()
        .iter()
        .map(|target| target.name.as_str())
        .collect();
    assert_eq!(names, ["CTL_a", "CTL_b"]);
    let mesh = part.model().lods.first().and_then(|lod| lod.meshes.first());
    let at = |stream: u32| {
        mesh.and_then(|mesh| mesh.source.iter().position(|&source| source == stream))
            .and_then(|vertex| u32::try_from(vertex).ok())
    };
    let first = part.morphs().first().ok_or("the first target")?;
    assert_eq!(first.vertices, [at(5).ok_or("stream vertex 5")?]);
    close(first.deltas[0][0], 0.05);
    close(first.deltas[0][1], 0.0);
    assert!(face_issues(&npc).is_empty(), "{:?}", npc.armour.issues());
    Ok(())
}

#[test]
fn a_face_animation_moves_the_joint_and_sets_the_weights_of_the_expressions() -> TestResult {
    let npc = build(npc_face_entries(), &Expressions::All)?;
    let models = ArchiveModels::new(&npc.archive);
    let list = [control("CTL_a")?];

    let animations = npc.armour.animate(&list, &models);

    assert!(animations.issues().is_empty(), "{:?}", animations.issues());
    let [face] = animations.sampled() else {
        return Err("one facial animation".into());
    };
    assert_eq!(face.times, [0.0, 10.0 / 30.0, 20.0 / 30.0]);
    let weights = &face.weights.first().ok_or("weights")?.values;
    assert_eq!(weights, &[0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);
    let [bone] = face.bones.as_slice() else {
        return Err("one joint moves".into());
    };
    close(bone.translations[1][0] - bone.translations[0][0], 0.01);
    close(bone.translations[2][0] - bone.translations[0][0], 0.0);

    let mut bytes = Vec::new();
    npc.armour
        .write_glb_animated(&animations, &GlbOptions::default(), &mut bytes)?;
    let glb = check_glb(&bytes)?;
    assert_eq!(glb.document.animations().count(), 1);
    Ok(())
}

#[test]
fn a_control_of_the_rotation_records_turns_the_joint() -> TestResult {
    let npc = build(npc_face_entries(), &Expressions::None)?;
    let models = ArchiveModels::new(&npc.archive);
    let list = [control("CTL_b")?];

    let animations = npc.armour.animate(&list, &models);

    let [face] = animations.sampled() else {
        return Err("one facial animation".into());
    };
    assert!(face.weights.is_empty(), "no expressions, no weights");
    let [bone] = face.bones.as_slice() else {
        return Err("one joint moves".into());
    };
    let turned = bone.rotations[1];
    let rest = bone.rotations[0];
    assert!(
        (turned[1] - rest[1]).abs() > 0.1,
        "{turned:?} against {rest:?}"
    );
    Ok(())
}

#[test]
fn without_expressions_the_head_has_its_joints_and_no_morph_targets() -> TestResult {
    let npc = build(npc_face_entries(), &Expressions::None)?;

    assert_eq!(head(&npc)?.morphs().len(), 0);
    assert!(face_issues(&npc).is_empty(), "{:?}", npc.armour.issues());
    Ok(())
}

#[test]
fn a_head_without_a_rig_logic_file_has_a_neutral_face() -> TestResult {
    let npc = build(npc_entries(), &Expressions::All)?;
    let models = ArchiveModels::new(&npc.archive);
    let list = [control("CTL_a")?];

    let animations = npc.armour.animate(&list, &models);

    assert_eq!(head(&npc)?.morphs().len(), 0);
    assert_eq!(face_issues(&npc).len(), 0);
    assert_eq!(animations.sampled().len(), 0);
    assert!(matches!(
        animations.issues().first().map(p4kx_assembly::Issue::kind),
        Some(IssueKind::AnimationNotUsed { .. })
    ));
    Ok(())
}

#[test]
fn an_unreadable_rig_logic_file_is_listed_and_the_head_is_still_built() -> TestResult {
    let npc = build(
        npc_face_entries_with(b"not a rig", &npc_face_shapes().build()),
        &Expressions::All,
    )?;

    assert_eq!(head(&npc)?.morphs().len(), 0);
    assert!(
        npc.armour
            .issues()
            .iter()
            .any(|issue| matches!(issue.kind(), IssueKind::FaceJointsFailed { .. })),
        "{:?}",
        npc.armour.issues()
    );
    Ok(())
}

#[test]
fn geometry_that_does_not_fit_the_rig_logic_costs_the_expressions_not_the_joints() -> TestResult {
    let mut shapes = TestSkinBlendshapes::small();
    shapes.vertices = npc_face_shapes().vertices;
    shapes.entries.clear();
    let bytes = npc_face_logic().build_rig();
    let npc = build(
        npc_face_entries_with(&bytes, &shapes.build()),
        &Expressions::All,
    )?;
    let models = ArchiveModels::new(&npc.archive);
    let list = [control("CTL_a")?];

    let animations = npc.armour.animate(&list, &models);

    assert_eq!(head(&npc)?.morphs().len(), 0);
    assert!(
        npc.armour
            .issues()
            .iter()
            .any(|issue| matches!(issue.kind(), IssueKind::MorphsFailed { .. })),
        "{:?}",
        npc.armour.issues()
    );
    assert_eq!(animations.sampled().len(), 1, "the joints still move");
    Ok(())
}

fn facial_bone_x(npc: &Npc) -> Result<f32, Box<dyn Error>> {
    let bone = npc
        .armour
        .skeleton()
        .bones
        .iter()
        .find(|bone| bone.name == NPC_FACIAL_BONE)
        .ok_or("the facial bone joined the skeleton")?;
    Ok(bone.relative.translation[0])
}

#[test]
fn the_facial_bone_sits_where_the_skeleton_file_of_the_head_puts_it() -> TestResult {
    let npc = build(npc_face_entries(), &Expressions::None)?;

    close(facial_bone_x(&npc)?, NPC_SKELETON_SHIFT);
    Ok(())
}

#[test]
fn without_a_skeleton_file_the_facial_bone_keeps_the_pose_of_the_skin() -> TestResult {
    let entries = npc_face_entries()
        .into_iter()
        .filter(|entry| !entry.path().eq_ignore_ascii_case(NPC_SKELETON_FILE))
        .collect();
    let npc = build(entries, &Expressions::None)?;

    close(facial_bone_x(&npc)?, 0.0);
    assert_eq!(face_issues(&npc).len(), 0);
    Ok(())
}

/// Builds the head with `skeleton` as its skeleton file and checks that the bone keeps the pose
/// of the skin and that the mismatch is listed as a face joints issue naming the bone.
fn assert_refused_skeleton(skeleton: &TestSkeleton, what: &str) -> TestResult {
    let npc = build(npc_face_entries_posed_by(skeleton), &Expressions::None)?;

    close(facial_bone_x(&npc)?, 0.0);
    let named = npc.armour.issues().iter().any(|issue| {
        matches!(
            issue.kind(),
            IssueKind::FaceJointsFailed { reason }
                if reason.contains(NPC_FACIAL_BONE) && reason.contains(what)
        )
    });
    assert!(named, "{what}: {:?}", npc.armour.issues());
    let models = ArchiveModels::new(&npc.archive);
    let list = [control("CTL_a")?];
    let animations = npc.armour.animate(&list, &models);
    assert_eq!(animations.sampled().len(), 1, "the joints still move");
    Ok(())
}

#[test]
fn a_skeleton_file_with_another_parent_for_the_facial_bone_is_refused() -> TestResult {
    let mut bones = npc_face_skeleton(NPC_SKELETON_SHIFT).bones().to_vec();
    bones.get_mut(3).ok_or("the facial bone")?.parent = Some(1);
    assert_refused_skeleton(&TestSkeleton::new(bones), "parent")
}

#[test]
fn a_skeleton_file_with_another_controller_id_for_the_facial_bone_is_refused() -> TestResult {
    let mut bones = npc_face_skeleton(NPC_SKELETON_SHIFT).bones().to_vec();
    bones.insert(
        3,
        TestBone {
            name: "extra".to_owned(),
            parent: Some(2),
            rotation: [0.0, 0.0, 0.0, 1.0],
            translation: [0.0; 3],
        },
    );
    assert_refused_skeleton(&TestSkeleton::new(bones), "controller ID")
}

#[test]
fn a_skeleton_file_without_the_facial_bone_is_refused() -> TestResult {
    let mut bones = npc_face_skeleton(NPC_SKELETON_SHIFT).bones().to_vec();
    bones.truncate(3);
    assert_refused_skeleton(&TestSkeleton::new(bones), "lacks")
}

#[test]
fn named_expressions_are_the_only_targets_and_an_unknown_name_is_listed() -> TestResult {
    let only = Expressions::Only(vec!["ctl_b".to_owned(), "nope".to_owned()]);
    let npc = build(npc_face_entries(), &only)?;

    let names: Vec<&str> = head(&npc)?
        .morphs()
        .iter()
        .map(|target| target.name.as_str())
        .collect();
    assert_eq!(names, ["CTL_b"]);
    assert!(npc.armour.issues().iter().any(|issue| matches!(
        issue.kind(),
        IssueKind::UnknownExpression { name } if name == "nope"
    )));
    Ok(())
}
