// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The `RigLogic` chunk of a skin is kept as raw bytes and counts as read.

use std::error::Error;

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_model::{ChunkUsage, CompanionError, CompanionPath, Model, ModelFile, RigLogicBytes};
use p4kx_testkit::{RIG_LOGIC_CHUNK, TestModel, TestRigLogic};

type TestResult = Result<(), Box<dyn Error>>;

fn read(model: &TestModel) -> Result<Model, Box<dyn Error>> {
    let file = ChunkFile::parse(model.cgf())?;
    let cgfm = model.cgfm();
    let mut companions = move |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(Some(cgfm.clone()))
    };
    Ok(Model::read("cube.cgf", &file, &mut companions)?)
}

#[test]
fn the_model_holds_the_bytes_of_its_rig_logic_chunk() -> TestResult {
    let chunk = TestRigLogic::small().build();
    let model = read(&TestModel::cube().with_chunk(RIG_LOGIC_CHUNK, chunk.clone()))?;

    assert_eq!(
        model.rig_logic.as_ref().map(RigLogicBytes::as_slice),
        Some(chunk.as_slice())
    );
    Ok(())
}

#[test]
fn a_model_without_the_chunk_has_none() -> TestResult {
    let model = read(&TestModel::cube())?;

    assert_eq!(model.rig_logic, None);
    Ok(())
}

#[test]
fn the_rig_logic_chunk_counts_as_read() -> TestResult {
    let model =
        read(&TestModel::cube().with_chunk(RIG_LOGIC_CHUNK, TestRigLogic::small().build()))?;

    let usage: Vec<ChunkUsage> = model
        .chunks
        .iter()
        .filter(|chunk| chunk.file == ModelFile::Model && chunk.chunk_type == ivo::RIG_LOGIC)
        .map(|chunk| chunk.usage)
        .collect();
    assert_eq!(usage, [ChunkUsage::Read]);
    Ok(())
}

#[test]
fn the_bytes_compare_by_content_and_debug_prints_only_the_length() {
    let first = RigLogicBytes::new(&[1, 2, 3]);

    assert_eq!(first, RigLogicBytes::new(&[1, 2, 3]));
    assert_ne!(first, RigLogicBytes::new(&[1, 2, 4]));
    assert_eq!(format!("{first:?}"), "RigLogicBytes(3 bytes)");
}
