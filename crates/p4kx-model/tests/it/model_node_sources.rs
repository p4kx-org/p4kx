// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ops::Range;

use p4kx_chunks::ChunkFile;
use p4kx_model::{
    CompanionError, CompanionPath, LodSelection, MaterialReading, Model, ModelNodeSources,
    PositionBox,
};
use p4kx_testkit::{TestChunkFile, TestCrchModel, TestModel, TestNode};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn read(bytes: &[u8]) -> TestResult<(Model, ModelNodeSources)> {
    let file = ChunkFile::parse(bytes)?;
    let mut absent = |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> { Ok(None) };
    let mut nodes = ModelNodeSources::default();
    let observed = Model::read_with_node_sources(
        "root.cgf",
        &file,
        &mut absent,
        LodSelection::All,
        PositionBox::default(),
        MaterialReading::Drawn,
        &mut nodes,
    )?;
    let ordinary = Model::read("root.cgf", &file, &mut absent)?;
    assert_eq!(
        ordinary, observed,
        "capturing coordinates leaves model interpretation unchanged"
    );
    Ok((observed, nodes))
}

fn at(bytes: &[u8], range: Range<u64>) -> TestResult<&[u8]> {
    bytes
        .get(usize::try_from(range.start)?..usize::try_from(range.end)?)
        .ok_or_else(|| "original field bounds".into())
}

#[test]
fn model_node_sources_hold_the_used_name_and_parent_words_in_both_layouts() -> TestResult {
    let node = TestNode {
        name: "place\nignored".to_owned(),
        parent: None,
        local: [
            [1.0, 0.0, 0.0, 4.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        kind: 2,
    };
    let ivo = TestModel::cube()
        .helpers_only()
        .with_nodes(vec![node])
        .cgf();
    let (model, nodes) = read(&ivo)?;
    assert_eq!(model.nodes.first().ok_or("Ivo helper")?.name, "place");
    let node = nodes.node(0).ok_or("Ivo original node")?;
    assert_eq!(at(&ivo, node.name().ok_or("Ivo name span")?)?, b"place\n");
    assert_eq!(at(&ivo, node.parent())?, u16::MAX.to_le_bytes());
    assert_eq!(at(&ivo, node.matrix())?.len(), 48);
    assert!(node.id().is_none(), "an Ivo row has no authored chunk ID");

    let crch = TestCrchModel::cube().bytes();
    let (_, nodes) = read(&crch)?;
    let root = nodes.node(0).ok_or("CrCh root node")?;
    let child = nodes.node(1).ok_or("CrCh child node")?;
    assert_eq!(
        at(&crch, child.name().ok_or("CrCh name span")?)?,
        b"cube_mesh\0"
    );
    assert_eq!(
        at(&crch, child.parent())?,
        at(&crch, root.id().ok_or("original root ID")?)?
    );
    assert_eq!(at(&crch, child.matrix())?.len(), 64);
    Ok(())
}

#[test]
fn synthetic_root_has_no_original_node_coordinates() -> TestResult {
    let bytes = TestChunkFile::ivo().bytes();
    let (model, nodes) = read(&bytes)?;
    assert_eq!(model.nodes.len(), 1, "decoder supplies the root");
    assert!(
        nodes.node(0).is_none(),
        "the generated root has no source bytes"
    );
    Ok(())
}
