// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::chunks::ChunkFile;
use p4kx_extraction::model::{CompanionError, CompanionPath, Model};
use p4kx_testkit::TestModel;

use crate::MeshNodeInput;

use super::{MeshNodeCapture, flatten};

#[test]
fn the_effective_skin_excludes_node_matrices_from_observed_vertices_and_bounds()
-> Result<(), Box<dyn Error>> {
    let file = ChunkFile::parse(TestModel::two_bone_skin().single_file().cgf())?;
    let mut absent = |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> { Ok(None) };
    let mut model = Model::read("skin", &file, &mut absent)?;
    for node in &mut model.nodes {
        node.transform.rows[0][3] = 25.0;
    }

    let observed = flatten(&model, 0, MeshNodeCapture::Enabled).ok_or("skinned flattening")?;
    let unobserved =
        flatten(&model, 0, MeshNodeCapture::Disabled).ok_or("unobserved flattening")?;
    assert_eq!(observed.vertices, unobserved.vertices);
    assert_eq!(observed.indices, unobserved.indices);
    assert_eq!(observed.bounds, unobserved.bounds);
    assert!(unobserved.mesh_node_trace.is_none());
    let trace = observed.mesh_node_trace.ok_or("skin observation")?;
    assert!(
        trace.ranges.iter().all(Option::is_none),
        "a skin uses skeleton space"
    );
    assert_eq!(
        trace.into_bound_inputs(),
        [] as [MeshNodeInput; 0],
        "a skin supplies no model-node bound"
    );

    model.bones.clear();
    let fallback = flatten(&model, 0, MeshNodeCapture::Enabled).ok_or("unbound flattening")?;
    assert_ne!(fallback.vertices, observed.vertices);
    assert_ne!(fallback.bounds, observed.bounds);
    let trace = fallback
        .mesh_node_trace
        .ok_or("static fallback observation")?;
    assert!(
        trace.ranges.iter().any(Option::is_some),
        "an unusable skeleton leaves the node matrix in use"
    );
    assert_eq!(trace.into_bound_inputs().len(), 1);
    Ok(())
}
