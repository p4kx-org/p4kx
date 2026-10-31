// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Heads and hair read from synthetic skins, for the tests of what follows the head.

use p4kx_chunks::ChunkFile;
use p4kx_model::{CompanionError, CompanionPath, Model};
use p4kx_testkit::{TestBone, TestModel, TestPositions, TestSkeleton, armour_male_skeleton};

/// The index of the head bone in the male skeleton.
pub(super) const HEAD: u16 = 2;
/// The vertices of the test cube.
pub(super) const VERTICES: usize = 24;

/// The result of a test.
pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// A bone named `name` below the bone `parent`, 10 cm above it.
pub(super) fn bone(name: &str, parent: u16) -> TestBone {
    TestBone {
        name: name.to_owned(),
        parent: Some(parent),
        rotation: [0.0, 0.0, 0.0, 1.0],
        translation: [0.0, 0.0, 0.1],
    }
}

/// The cube as a skin of the male body plus `extra`, every vertex weighted to `bone`.
pub(super) fn skin(extra: Vec<TestBone>, bone: Option<u16>) -> TestModel {
    let mut bones = armour_male_skeleton().bones().to_vec();
    bones.extend(extra);
    let cube = TestModel::cube()
        .with_positions(TestPositions::Float)
        .with_nodes(Vec::new())
        .with_skeleton(&TestSkeleton::new(bones));
    match bone {
        Some(bone) => cube.with_bone_map(false, vec![vec![(bone, 255)]; VERTICES]),
        None => cube,
    }
}

/// Reads `source` as `<stem>.skin` with its levels of detail.
pub(super) fn read(source: &TestModel, stem: &str) -> TestResult<Model> {
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling(name) if name == format!("{stem}.skinm") => Some(source.cgfm()),
            CompanionPath::Sibling(name) if name == format!("{stem}_lod1.skinm") => {
                source.lod_cgfm(1)
            }
            CompanionPath::Sibling(name) if name == format!("{stem}_lod1.skin") => {
                source.lod_cgf(1)
            }
            _ => None,
        })
    };
    Ok(Model::read(
        &format!("{stem}.skin"),
        &ChunkFile::parse(source.cgf())?,
        &mut companions,
    )?)
}

/// A head with a jaw bone, every vertex weighted to the jaw, of two levels of detail.
pub(super) fn head_with_jaw() -> TestResult<Model> {
    let extra = vec![bone("jaw", HEAD), bone("tooth", 3)];
    let source = skin(extra.clone(), Some(3)).with_lod(skin(extra, Some(3)));
    read(&source, "head")
}

/// The head as it is read at `level`: its first level of detail is that level.
pub(super) fn head_at(level: usize) -> TestResult<Model> {
    let mut head = head_with_jaw()?;
    head.lods.drain(..level);
    Ok(head)
}

/// The hair as it is read at `level`: its first level of detail is that level.
pub(super) fn hair_at(level: usize) -> TestResult<Model> {
    let mut hair = hair()?;
    hair.lods.drain(..level);
    Ok(hair)
}

/// A hair of two levels of detail, every vertex weighted to the head bone.
pub(super) fn hair() -> TestResult<Model> {
    let source = skin(Vec::new(), Some(HEAD)).with_lod(skin(Vec::new(), Some(HEAD)));
    read(&source, "hair")
}
