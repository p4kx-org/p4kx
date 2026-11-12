// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reading the skeletons of the testkit skins and static models through `p4kx-model`.

use std::error::Error;

use p4kx_chunks::ChunkFile;
use p4kx_model::{
    Animation, BoneTransform, ChunkUsage, CompanionError, CompanionPath, ControllerEligibility,
    CorruptChunk, Model, ModelError, ModelFile, SkeletonSource, chained_world_poses,
};
use p4kx_testkit::{TestAnimation, TestBone, TestChunkFile, TestModel, TestSkeleton, TestTrack};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

const COMPILED_BONES_2: u32 = 0xC201_1111;
const COMPILED_BONES: u32 = 0xC201_973C;

fn read(model: &TestModel) -> Result<Model, ModelError> {
    let cgfm = model.cgfm();
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("cube.cgfm") => Some(cgfm.clone()),
            _ => None,
        })
    };
    let file = ChunkFile::parse(model.cgf()).map_err(|source| ModelError::CompanionChunks {
        file: "cube.cgf".to_owned(),
        source,
    })?;
    Model::read("cube.cgf", &file, &mut companions)
}

fn assert_close(actual: &[f32], expected: &[f32], what: &str) {
    assert_eq!(actual.len(), expected.len(), "{what}: length");
    for (a, e) in actual.iter().zip(expected) {
        assert!(
            (a - e).abs() < 1e-5,
            "{what}: {actual:?} is not {expected:?}"
        );
    }
}

const fn corrupt_source(error: &ModelError) -> Option<&CorruptChunk> {
    match error {
        ModelError::Corrupt { source, .. } => Some(source),
        _ => None,
    }
}

#[test]
fn a_skin_has_its_bones_with_parents_and_bind_poses() -> TestResult {
    let skeleton = TestSkeleton::two_bones();
    let model = read(&TestModel::two_bone_skin())?;

    let names: Vec<&str> = model.bones.iter().map(|bone| bone.name.as_str()).collect();
    assert_eq!(names, ["root", "tip"], "bone names in order");
    let parents: Vec<Option<usize>> = model.bones.iter().map(|bone| bone.parent).collect();
    assert_eq!(parents, [None, Some(0)], "the tip hangs below the root");
    for ((bone, source), world) in model
        .bones
        .iter()
        .zip(skeleton.bones())
        .zip(skeleton.world())
    {
        assert_close(
            &bone.relative.rotation,
            &source.rotation,
            "relative rotation",
        );
        assert_close(
            &bone.relative.translation,
            &source.translation,
            "relative translation",
        );
        assert_close(&bone.world.rotation, &world.0, "world rotation");
        assert_close(&bone.world.translation, &world.1, "world translation");
        assert_eq!(bone.node, None, "a skin's bones drive no node");
    }
    let ids: Vec<u32> = model.bones.iter().map(|bone| bone.controller_id).collect();
    assert_eq!(ids, [0x9000_0000, 0x9000_0001], "controller IDs");
    assert_close(
        &model.bones[1].world.translation,
        &[0.0, 0.0, 2.0],
        "the tip stands 2 m above the root",
    );
    Ok(())
}

#[test]
fn the_skeleton_chunk_is_read_and_a_model_without_one_has_no_bones() -> TestResult {
    let skin = read(&TestModel::two_bone_skin())?;
    let bones = skin
        .chunks
        .iter()
        .find(|chunk| chunk.chunk_type.id() == COMPILED_BONES_2)
        .ok_or("a CompiledBones2 chunk")?;
    assert_eq!(bones.file, ModelFile::Model, "it is in the model file");
    assert_eq!(bones.usage, ChunkUsage::Read, "the model read it");

    let cube = read(&TestModel::cube())?;
    assert!(cube.bones.is_empty(), "a static cube has no bones");
    Ok(())
}

#[test]
fn a_static_model_with_animated_parts_has_the_bones_of_its_compact_chunk() -> TestResult {
    let skeleton = TestSkeleton::chain(3);
    let model = read(&TestModel::cube().with_compact_skeleton(&skeleton))?;

    let names: Vec<&str> = model.bones.iter().map(|bone| bone.name.as_str()).collect();
    assert_eq!(names, ["bone_0", "bone_1", "bone_2"], "bone names");
    let parents: Vec<Option<usize>> = model.bones.iter().map(|bone| bone.parent).collect();
    assert_eq!(parents, [None, Some(0), Some(1)], "a chain");
    let nodes: Vec<Option<usize>> = model.bones.iter().map(|bone| bone.node).collect();
    assert_eq!(
        nodes,
        [Some(0), Some(1), Some(2)],
        "the node each bone drives"
    );
    for ((bone, source), world) in model
        .bones
        .iter()
        .zip(skeleton.bones())
        .zip(skeleton.world())
    {
        assert_close(
            &bone.relative.rotation,
            &source.rotation,
            "relative rotation",
        );
        assert_close(&bone.world.translation, &world.1, "world translation");
    }
    Ok(())
}

#[test]
fn bone_names_are_kept_aligned_with_their_bones_when_one_is_empty() -> TestResult {
    let bone = |name: &str, parent| TestBone {
        name: name.to_owned(),
        parent,
        rotation: [0.0, 0.0, 0.0, 1.0],
        translation: [0.0; 3],
    };
    let skeleton = TestSkeleton::new(vec![
        bone("root", None),
        bone("", Some(0)),
        bone("last", Some(0)),
    ]);
    for model in [
        TestModel::cube().with_skeleton(&skeleton),
        TestModel::cube().with_compact_skeleton(&skeleton),
    ] {
        let names: Vec<String> = read(&model)?
            .bones
            .into_iter()
            .map(|bone| bone.name)
            .collect();
        assert_eq!(names, ["root", "", "last"], "the empty name stays in place");
    }
    Ok(())
}

#[test]
fn a_corrupt_skeleton_is_a_typed_error_never_a_panic() -> TestResult {
    let bone = |parent| TestBone {
        name: "bone".to_owned(),
        parent,
        rotation: [0.0, 0.0, 0.0, 1.0],
        translation: [0.0; 3],
    };
    let two = TestSkeleton::two_bones();
    let full = two.compiled_bones_2();
    let compact = two.compiled_bones();
    let cases: [(&str, u32, Vec<u8>); 7] = [
        (
            "truncated",
            COMPILED_BONES_2,
            full.get(..100).unwrap_or_default().to_vec(),
        ),
        (
            "a name missing",
            COMPILED_BONES_2,
            full.get(..full.len() - 4).unwrap_or_default().to_vec(),
        ),
        (
            "a parent that does not exist",
            COMPILED_BONES_2,
            TestSkeleton::new(vec![bone(Some(7))]).compiled_bones_2(),
        ),
        (
            "its own parent",
            COMPILED_BONES_2,
            TestSkeleton::new(vec![bone(Some(0))]).compiled_bones_2(),
        ),
        (
            "a cycle",
            COMPILED_BONES_2,
            TestSkeleton::new(vec![bone(Some(1)), bone(Some(0))]).compiled_bones_2(),
        ),
        (
            "compact truncated",
            COMPILED_BONES,
            compact.get(..60).unwrap_or_default().to_vec(),
        ),
        (
            "compact cycle",
            COMPILED_BONES,
            TestSkeleton::new(vec![bone(Some(1)), bone(Some(0))]).compiled_bones(),
        ),
    ];
    for (what, chunk_type, data) in cases {
        let error = read(&TestModel::cube().with_chunk(chunk_type, data))
            .err()
            .ok_or_else(|| format!("{what} is accepted"))?;
        assert!(
            corrupt_source(&error).is_some(),
            "{what}: {error:?} is no corrupt chunk"
        );
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn arbitrary_and_mutated_skeleton_chunks_never_panic(
        noise in proptest::collection::vec(any::<u8>(), 0..300),
        at in 0_usize..200,
        patch in proptest::collection::vec(any::<u8>(), 1..8),
        compact in any::<bool>(),
        mutate in any::<bool>(),
    ) {
        let skeleton = TestSkeleton::chain(3);
        let (chunk_type, mut data) = if compact {
            (COMPILED_BONES, skeleton.compiled_bones())
        } else {
            (COMPILED_BONES_2, skeleton.compiled_bones_2())
        };
        if mutate {
            for (slot, byte) in data.iter_mut().skip(at).zip(&patch) {
                *slot = *byte;
            }
        } else {
            data = noise;
        }
        let _outcome = read(&TestModel::cube().with_chunk(chunk_type, data));
    }
}

#[test]
fn a_bone_count_beyond_the_chunk_does_not_allocate_it() -> TestResult {
    let mut data = TestSkeleton::two_bones().compiled_bones_2();
    data.splice(..4, u32::MAX.to_le_bytes());
    let error = read(&TestModel::cube().with_chunk(COMPILED_BONES_2, data))
        .err()
        .ok_or("a bone count of four billion is accepted")?;
    assert!(
        matches!(corrupt_source(&error), Some(CorruptChunk::Truncated(_))),
        "{error:?}"
    );
    Ok(())
}

#[test]
fn chained_poses_follow_the_relative_poses_whatever_the_stored_world_poses_say() -> TestResult {
    let skeleton = TestSkeleton::chain(4);
    let mut model = read(&TestModel::cube().with_skeleton(&skeleton))?;
    for bone in &mut model.bones {
        bone.world = BoneTransform::IDENTITY;
    }

    let poses = chained_world_poses(&model.bones);

    for (pose, (rotation, position)) in poses.iter().zip(skeleton.world()) {
        assert_close(&pose.rotation, &rotation, "chained rotation");
        assert_close(&pose.translation, &position, "chained translation");
    }
    Ok(())
}

#[test]
fn chained_poses_treat_a_missing_parent_and_a_cycle_as_roots() -> TestResult {
    let mut model = read(&TestModel::two_bone_skin())?;
    model.bones[0].parent = Some(1);
    let cycle = chained_world_poses(&model.bones);
    assert_eq!(cycle.len(), 2, "a pose per bone");
    let (first, second) = (model.bones[0].relative, model.bones[1].relative);
    assert_close(
        &cycle[1].translation,
        &second.translation,
        "the bone that closes the cycle is a root",
    );
    let below = second.then(&first);
    assert_close(&cycle[0].translation, &below.translation, "its child");
    assert_close(&cycle[0].rotation, &below.rotation, "its child's rotation");

    model.bones[0].parent = Some(99);
    let missing = chained_world_poses(&model.bones);
    assert_close(
        &missing[0].translation,
        &model.bones[0].relative.translation,
        "the bone is a root",
    );
    Ok(())
}

#[test]
fn admitted_skeleton_spans_and_original_ordinals_agree_with_the_model_binder() -> TestResult {
    let skeleton = TestSkeleton::two_bones();
    for (kind, mut encoded, second_id_at) in [
        (COMPILED_BONES, skeleton.compiled_bones(), 64),
        (COMPILED_BONES_2, skeleton.compiled_bones_2(), 72),
    ] {
        let id = TestSkeleton::controller_id(0);
        encoded
            .get_mut(second_id_at..second_id_at + 4)
            .ok_or("second ID field")?
            .copy_from_slice(&id.to_le_bytes());
        let bytes = TestChunkFile::ivo()
            .chunk(kind, 1, encoded.as_slice())
            .bytes();
        let file = ChunkFile::parse(&bytes)?;
        let source = SkeletonSource::read("source.chr", &file)?.ok_or("original skeleton")?;
        assert_eq!(source.bone_count(), 2, "both physical bones are retained");
        let chunk = file.chunks().first().ok_or("original table entry")?;
        let first_id_at = if kind == COMPILED_BONES { 48 } else { 4 };
        for index in 0..source.bone_count() {
            let range = source
                .controller_id_range(index)
                .ok_or("actual controller bytes")?;
            let relative = if index == 0 {
                first_id_at
            } else {
                second_id_at
            };
            assert_eq!(
                range.start,
                chunk.offset() + u64::try_from(relative)?,
                "the physically distinct bone ID position",
            );
            let range = usize::try_from(range.start)?..usize::try_from(range.end)?;
            assert_eq!(
                bytes.get(range),
                Some(id.to_le_bytes().as_slice()),
                "original ID bytes"
            );
        }
        assert_binding_parity(&source, kind, encoded, id)?;
    }
    Ok(())
}

fn assert_binding_parity(
    source: &SkeletonSource,
    kind: u32,
    encoded: Vec<u8>,
    id: u32,
) -> TestResult {
    let animation = TestAnimation::new("track.caf", 30)
        .track(TestTrack::new(id))
        .track(TestTrack::new(id).positions(&[(0, [0.0; 3])]))
        .track(TestTrack::new(id).positions(&[(0, [1.0; 3])]));
    let decoded = Animation::read_caf("track.caf", &ChunkFile::parse(animation.caf())?)?;
    let model = read(&TestModel::cube().with_chunk(kind, encoded))?;
    let known = decoded.bind(&model.bones);
    let mut binder = source.binder();
    let empty = binder.observe(0, id, ControllerEligibility::Empty);
    let first = binder.observe(1, id, ControllerEligibility::Track);
    let duplicate = binder.observe(2, id, ControllerEligibility::Track);
    assert_eq!(empty.bound_bone(), None, "the empty occurrence cannot bind");
    assert_eq!(
        first.first_controller(),
        0,
        "the empty occurrence is still first authored"
    );
    assert_eq!(
        first.first_eligible(),
        Some(1),
        "first key-bearing source ordinal"
    );
    assert_eq!(first.matching_bones(), 2, "original skeleton collision");
    assert_eq!(
        first.bound_bone(),
        known.bound.first().map(|&(bone, _)| bone),
        "same first bone"
    );
    assert_eq!(duplicate.bound_bone(), None, "only the first track binds");
    assert_eq!(
        known.unbound,
        [id],
        "the legacy result reports the duplicate track"
    );
    Ok(())
}
