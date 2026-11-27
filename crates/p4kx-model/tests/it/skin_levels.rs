// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The levels of detail of a skin are weighted to the skeleton of their own `_lod<n>` file.

use std::error::Error;

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_model::{
    ChunkUsage, CompanionError, CompanionPath, CorruptChunk, LodSelection, LodSkeletonOrigin,
    Model, ModelError, ModelFile,
};
use p4kx_testkit::{TestChunkFile, TestModel, TestSkeleton};

type TestResult = Result<(), Box<dyn Error>>;

const UNKNOWN_CHUNK: u32 = 0xDEAD_BEEF;

fn read(model: &TestModel, with_lod_file: bool, lods: LodSelection) -> Result<Model, ModelError> {
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("skin.skinm") => Some(model.cgfm()),
            CompanionPath::Sibling("skin_lod1.skinm") => model.lod_cgfm(1),
            CompanionPath::Sibling("skin_lod1.skin") if with_lod_file => model.lod_cgf(1),
            _ => None,
        })
    };
    let file = ChunkFile::parse(model.cgf()).map_err(|source| ModelError::CompanionChunks {
        file: "skin.skin".to_owned(),
        source,
    })?;
    Model::read_with("skin.skin", &file, &mut companions, lods)
}

fn base() -> TestModel {
    TestModel::two_bone_skin()
}

fn level_one(skeleton: &TestSkeleton, joint: u16) -> TestModel {
    TestModel::cube()
        .with_skeleton(skeleton)
        .with_bone_map(false, vec![vec![(joint, 255)]; 24])
}

#[test]
fn a_level_with_its_own_skeleton_is_weighted_to_it() -> TestResult {
    let chain = TestSkeleton::chain(6);
    let skin = base().with_lod(level_one(&chain, 5));
    let model = read(&skin, true, LodSelection::All)?;

    let [lod0, lod1] = model.lods.as_slice() else {
        return Err("two levels".into());
    };
    assert_eq!(model.bones.len(), 2, "the model file has two bones");
    assert!(lod0.bones.is_none(), "level 0 uses the model's bones");
    assert_eq!(model.bones_of(lod0).len(), 2, "two bones at level 0");
    assert_eq!(
        lod1.bones.as_ref().map(Vec::len),
        Some(6),
        "level 1 has the six bones of its file"
    );
    assert_eq!(model.bones_of(lod1).len(), 6, "six bones at level 1");
    Ok(())
}

#[test]
fn a_level_with_the_same_skeleton_shares_the_models() -> TestResult {
    let skin = base().with_lod(level_one(&TestSkeleton::two_bones(), 1));
    let model = read(&skin, true, LodSelection::All)?;

    let lod1 = model.lods.get(1).ok_or("level 1")?;
    assert!(lod1.bones.is_none(), "equal skeletons are kept once");
    Ok(())
}

#[test]
fn a_level_alone_still_finds_its_skeleton() -> TestResult {
    let skin = base().with_lod(level_one(&TestSkeleton::chain(6), 5));
    let model = read(&skin, true, LodSelection::Only(1))?;

    let lod1 = model.lods.first().ok_or("level 1")?;
    assert_eq!(model.bones_of(lod1).len(), 6, "the bones of level 1");
    assert_eq!(model.bones.len(), 2, "the model's bones are read too");
    Ok(())
}

#[test]
fn a_bone_beyond_the_skeleton_of_the_level_is_corrupt() {
    let skin = base().with_lod(level_one(&TestSkeleton::chain(6), 6));

    let error = read(&skin, true, LodSelection::All).err();
    assert!(
        matches!(
            &error,
            Some(ModelError::Corrupt {
                source: CorruptChunk::JointRange {
                    joint: 6,
                    bones: 6,
                    ..
                },
                ..
            })
        ),
        "{error:?}"
    );
}

#[test]
fn a_level_without_a_file_of_its_own_is_checked_against_the_models_bones() {
    let skin = base().with_lod(level_one(&TestSkeleton::chain(6), 5));

    let error = read(&skin, false, LodSelection::All).err();
    assert!(
        matches!(
            &error,
            Some(ModelError::Corrupt {
                source: CorruptChunk::JointRange {
                    joint: 5,
                    bones: 2,
                    ..
                },
                ..
            })
        ),
        "{error:?}"
    );
}

#[test]
fn inherited_bones_retain_the_actual_level_file_observation() -> TestResult {
    let skeleton = TestSkeleton::two_bones();
    let reference = read(
        &base().with_lod(level_one(&skeleton, 1)),
        true,
        LodSelection::All,
    )?;
    let reference = reference.lods.get(1).ok_or("reference level")?;
    let no_skeleton = TestModel::cube().with_bone_map(false, vec![vec![(1, 255)]; 24]);
    let cases = [
        (LodSkeletonOrigin::EqualModel, level_one(&skeleton, 1), true),
        (
            LodSkeletonOrigin::MissingFile,
            level_one(&skeleton, 1),
            false,
        ),
        (LodSkeletonOrigin::NoSkeleton, no_skeleton, true),
        (
            LodSkeletonOrigin::EmptySkeleton,
            level_one(&TestSkeleton::new(Vec::new()), 1),
            true,
        ),
    ];
    for (expected, level, present) in cases {
        let model = read(&base().with_lod(level), present, LodSelection::All)?;
        let lod = model.lods.get(1).ok_or("level one")?;
        assert_eq!(
            lod.skeleton_origin, expected,
            "source observation remains distinct"
        );
        assert!(
            lod.bones.is_none(),
            "effective bones still share the model skeleton"
        );
        assert_eq!(
            model.bones_of(lod),
            model.bones.as_slice(),
            "effective skeleton is unchanged"
        );
        assert_eq!(
            lod.meshes, reference.meshes,
            "source observations do not change geometry"
        );
    }
    Ok(())
}

#[test]
fn level_model_chunks_keep_original_order_and_actual_usage() -> TestResult {
    let level = level_one(&TestSkeleton::chain(6), 5).with_chunk(UNKNOWN_CHUNK, [7, 8, 9]);
    let skin = base().with_lod(level);
    let bytes = skin.lod_cgf(1).ok_or("level model bytes")?;
    let file = ChunkFile::parse(bytes)?;
    let model = read(&skin, true, LodSelection::All)?;
    let lod = model.lods.get(1).ok_or("level one")?;
    assert_eq!(
        lod.skeleton_origin,
        LodSkeletonOrigin::Distinct,
        "distinct source is retained"
    );
    assert_eq!(
        model.bones_of(lod).len(),
        6,
        "level meshes keep their own skeleton"
    );
    let chunks: Vec<_> = model
        .chunks
        .iter()
        .filter(|chunk| chunk.file == ModelFile::LodModel(1))
        .collect();
    assert_eq!(
        chunks.len(),
        file.chunks().len(),
        "every actually read file chunk is inventoried"
    );
    for ((index, expected), actual) in file.chunks().iter().enumerate().zip(&chunks) {
        assert_eq!(
            (actual.index, actual.chunk_type),
            (index, expected.chunk_type()),
            "physical table ordinal and type remain exact"
        );
        let expected_usage = if expected.chunk_type() == ivo::COMPILED_BONES_2 {
            ChunkUsage::Read
        } else if expected.chunk_type().is_known() {
            ChunkUsage::Skipped
        } else {
            ChunkUsage::Unknown
        };
        assert_eq!(
            actual.usage, expected_usage,
            "usage belongs to the level model file"
        );
    }
    assert!(
        chunks
            .iter()
            .any(|chunk| chunk.chunk_type.id() == UNKNOWN_CHUNK
                && chunk.usage == ChunkUsage::Unknown),
        "unknown source chunk survives"
    );
    Ok(())
}

#[test]
fn an_unskinned_level_does_not_claim_a_skeleton_file_read() -> TestResult {
    let model = read(&base().with_lod(TestModel::cube()), true, LodSelection::All)?;
    let lod = model.lods.get(1).ok_or("level one")?;
    assert_eq!(
        lod.skeleton_origin,
        LodSkeletonOrigin::NotRead,
        "unrequested input is not missing"
    );
    assert!(
        model
            .chunks
            .iter()
            .all(|chunk| chunk.file != ModelFile::LodModel(1)),
        "an unopened file has no invented chunk table"
    );
    assert_eq!(
        model.lods.first().ok_or("base level")?.skeleton_origin,
        LodSkeletonOrigin::ModelFile,
        "base skeleton remains model-owned"
    );
    Ok(())
}

#[test]
fn a_relabelled_base_level_keeps_the_actual_model_skeleton_origin() -> TestResult {
    let model = read(&base(), true, LodSelection::OnlyOrBase(5))?;
    let lod = model.lods.first().ok_or("fallback level")?;
    assert_eq!(
        lod.level, 5,
        "existing fallback label remains requested level"
    );
    assert_eq!(
        lod.skeleton_origin,
        LodSkeletonOrigin::ModelFile,
        "fallback uses the actual base source"
    );
    assert_eq!(
        model.bones_of(lod),
        model.bones.as_slice(),
        "fallback skeleton is unchanged"
    );
    assert!(
        model
            .chunks
            .iter()
            .all(|chunk| chunk.file != ModelFile::LodModel(5)),
        "no unavailable level file is invented"
    );
    Ok(())
}

#[test]
fn an_unreadable_level_skeleton_is_not_reported_as_missing() {
    let level = TestModel::cube()
        .with_bone_map(false, vec![vec![(1, 255)]; 24])
        .with_chunk(ivo::COMPILED_BONES_2.id(), 1_u32.to_le_bytes());
    let result = read(&base().with_lod(level), true, LodSelection::All);
    assert!(
        matches!(result, Err(ModelError::Corrupt { ref file, chunk: "CompiledBones2", .. }) if file == "skin_lod1.skin"),
        "original typed reader failure remains an error"
    );
}

#[test]
fn a_visibility_only_level_retains_every_chunk_without_inventing_geometry() -> TestResult {
    const IVO_VERSION: u32 = 0x900;
    let fixture = base();
    let areas = TestChunkFile::ivo()
        .chunk(ivo::VIS_AREAS.id(), IVO_VERSION, [0; 64])
        .chunk(UNKNOWN_CHUNK, IVO_VERSION, [7, 8, 9])
        .bytes();
    let file = ChunkFile::parse(fixture.cgf())?;
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("skin.skinm") => Some(fixture.cgfm()),
            CompanionPath::Sibling("skin_lod1.skinm") => Some(areas.clone()),
            _ => None,
        })
    };
    let model = Model::read("skin.skin", &file, &mut companions)?;
    assert_eq!(model.lods.len(), 1, "only actual base geometry is retained");
    assert_eq!(
        model.lods[0].level, 0,
        "the existing base level is unchanged"
    );
    let chunks: Vec<_> = model
        .chunks
        .iter()
        .filter(|chunk| chunk.file == ModelFile::LodGeometry(1))
        .map(|chunk| (chunk.index, chunk.chunk_type.id(), chunk.usage))
        .collect();
    assert_eq!(
        chunks,
        [
            (0, ivo::VIS_AREAS.id(), ChunkUsage::Skipped),
            (1, UNKNOWN_CHUNK, ChunkUsage::Unknown),
        ],
        "the actual table keeps its order and source dispositions"
    );
    assert!(
        model
            .chunks
            .iter()
            .all(|chunk| chunk.file != ModelFile::LodModel(1)),
        "no unread level skeleton is invented"
    );
    Ok(())
}
