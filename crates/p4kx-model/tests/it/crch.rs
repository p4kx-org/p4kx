// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reading the testkit `CrCh` cube through the public API of `p4kx-model`.

use std::error::Error;

use p4kx_chunks::{ChunkFile, ChunkType, Container};
use p4kx_model::{
    ChunkUsage, CompanionError, CompanionPath, LodSelection, MaterialStatus, Model, ModelError,
};
use p4kx_testkit::{
    CRCH_CUBE_ROOT_OFFSET, CUBE_MATERIAL_LIBRARY, CUBE_MESH_OFFSET, TestChunkFile, TestCrchModel,
    TestModel,
};

type TestResult = Result<(), Box<dyn Error>>;

fn read_bytes(bytes: Vec<u8>, lods: LodSelection) -> Result<Model, Box<dyn Error>> {
    let file = ChunkFile::parse(bytes)?;
    let mut companions =
        |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> { Ok(None) };
    Ok(Model::read_with("cube.cgf", &file, &mut companions, lods)?)
}

fn read(model: &TestCrchModel) -> Result<Model, Box<dyn Error>> {
    read_bytes(model.bytes(), LodSelection::All)
}

fn assert_close(actual: [f32; 3], expected: [f32; 3], what: &str) {
    for (a, e) in actual.iter().zip(expected) {
        assert!(
            (a - e).abs() < 1e-4,
            "{what}: {actual:?} is not {expected:?}"
        );
    }
}

#[test]
fn cube_nodes_keep_names_hierarchy_and_local_transforms() -> TestResult {
    let model = read(&TestCrchModel::cube())?;

    let names: Vec<&str> = model.nodes.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(names, ["cube", "cube_mesh"], "node names in table order");
    let parents: Vec<Option<usize>> = model.nodes.iter().map(|node| node.parent).collect();
    assert_eq!(parents, [None, Some(0)], "cube_mesh hangs below cube");
    let [root, mesh_node] = model.nodes.as_slice() else {
        return Err("two nodes".into());
    };
    assert_close(
        root.transform.translation(),
        CRCH_CUBE_ROOT_OFFSET,
        "the root keeps its matrix",
    );
    assert_close(
        mesh_node.transform.translation(),
        CUBE_MESH_OFFSET,
        "the geometry node keeps its local matrix",
    );
    Ok(())
}

#[test]
fn cube_lod0_has_positions_indices_and_one_submesh() -> TestResult {
    let cube = TestCrchModel::cube();
    let model = read(&cube)?;

    assert_eq!(model.lods.len(), 1, "one level of detail");
    let lod = model.lods.first().ok_or("LOD0")?;
    assert_eq!(lod.level, 0, "level 0");
    let [mesh] = lod.meshes.as_slice() else {
        return Err("one mesh".into());
    };
    assert_eq!(mesh.node, 1, "the mesh belongs to cube_mesh");
    assert_eq!(mesh.positions.len(), 24, "every vertex");
    assert_eq!(lod.triangle_count(), 12, "twelve triangles");
    for (position, vertex) in mesh.positions.iter().zip(cube.vertices()) {
        assert_close(*position, vertex.position, "position");
    }
    assert_eq!(mesh.indices, cube.indices(), "the triangle list");
    assert!(
        mesh.normals_computed && mesh.uvs.is_empty(),
        "the physics mesh stores neither normals nor texture coordinates"
    );
    let [submesh] = mesh.submeshes.as_slice() else {
        return Err("one submesh".into());
    };
    assert_eq!(
        (
            submesh.material_id,
            submesh.first_index,
            submesh.index_count
        ),
        (0, 0, 36),
        "one submesh over all indices"
    );
    Ok(())
}

#[test]
fn cube_names_its_material_library() -> TestResult {
    let model = read(&TestCrchModel::cube())?;
    assert_eq!(
        model.material_library.as_deref(),
        Some(CUBE_MATERIAL_LIBRARY),
        "the MtlName chunk"
    );
    assert!(
        matches!(&model.materials, MaterialStatus::Missing { path, .. } if path == CUBE_MATERIAL_LIBRARY),
        "no resolver file: {:?}",
        model.materials
    );
    assert_eq!(model.geometry, None, "no geometry companion");
    assert_eq!(model.material_ids(), [0], "the submesh's material ID");
    Ok(())
}

#[test]
fn the_structure_matches_the_ivo_cube() -> TestResult {
    let crch = read(&TestCrchModel::cube())?;
    let ivo_model = TestModel::cube();
    let cgfm = ivo_model.cgfm();
    let file = ChunkFile::parse(ivo_model.cgf())?;
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("cube.cgfm") => Some(cgfm.clone()),
            _ => None,
        })
    };
    let ivo = Model::read("cube.cgf", &file, &mut companions)?;

    let shape = |model: &Model| {
        (
            model
                .nodes
                .iter()
                .map(|node| node.parent)
                .collect::<Vec<_>>(),
            model
                .nodes
                .iter()
                .map(|node| node.name.clone())
                .collect::<Vec<_>>(),
            model.lods.len(),
            model
                .lods
                .iter()
                .map(|lod| (lod.meshes.len(), lod.vertex_count(), lod.triangle_count()))
                .collect::<Vec<_>>(),
            model.material_library.clone(),
        )
    };
    assert_eq!(
        shape(&crch),
        shape(&ivo),
        "nodes, meshes and material library"
    );
    let crch_mesh = crch
        .lods
        .first()
        .and_then(|lod| lod.meshes.first())
        .ok_or("mesh")?;
    let ivo_mesh = ivo
        .lods
        .first()
        .and_then(|lod| lod.meshes.first())
        .ok_or("mesh")?;
    assert_eq!(crch_mesh.node, ivo_mesh.node, "the same node");
    assert_eq!(crch_mesh.indices, ivo_mesh.indices, "the same triangles");
    for (a, b) in crch_mesh.positions.iter().zip(&ivo_mesh.positions) {
        assert_close(*a, *b, "position");
    }
    Ok(())
}

#[test]
fn unknown_chunks_are_skipped_and_reported_and_read_ones_are_marked() -> TestResult {
    let model = read(&TestCrchModel::cube())?;

    let unknown: Vec<_> = model.unknown_chunks().collect();
    let [only] = unknown.as_slice() else {
        return Err("one unknown chunk".into());
    };
    assert_eq!(
        only.chunk_type,
        ChunkType::new(Container::CrCh, 0x300A),
        "its type"
    );
    let usage = |type_id: u32| {
        model
            .chunks
            .iter()
            .filter(|chunk| chunk.chunk_type.id() == type_id)
            .map(|chunk| chunk.usage)
            .collect::<Vec<_>>()
    };
    assert_eq!(usage(0x100B), [ChunkUsage::Read; 2], "both nodes");
    assert_eq!(usage(0x1000), [ChunkUsage::Read], "the mesh header");
    assert_eq!(usage(0x1018), [ChunkUsage::Read], "the physics data");
    assert_eq!(usage(0x1014), [ChunkUsage::Read], "the material name");
    assert_eq!(
        usage(0x1001),
        [ChunkUsage::Skipped],
        "the helper is known and unread"
    );
    Ok(())
}

#[test]
fn a_crch_model_has_only_lod0() -> TestResult {
    let only0 = read_bytes(TestCrchModel::cube().bytes(), LodSelection::Only(0))?;
    assert_eq!(only0.lods.len(), 1, "LOD0");
    let error = read_bytes(TestCrchModel::cube().bytes(), LodSelection::Only(1))
        .err()
        .ok_or("an error")?;
    assert!(
        matches!(
            error.downcast_ref::<ModelError>(),
            Some(ModelError::NoSuchLod { lod: 1, .. })
        ),
        "{error:?}"
    );
    Ok(())
}

#[test]
fn a_file_without_nodes_has_a_root_node_and_no_meshes() -> TestResult {
    let bytes = TestChunkFile::crch()
        .chunk(0x1001, 0x744, vec![0; 16])
        .bytes();
    let model = read_bytes(bytes, LodSelection::All)?;
    assert_eq!(model.nodes.len(), 1, "the root node named after the file");
    assert_eq!(
        model.lods.first().map(|lod| lod.meshes.len()),
        Some(0),
        "no meshes"
    );
    Ok(())
}

#[test]
fn physics_count_sources_follow_emitted_meshes_and_exclude_other_read_chunks() -> TestResult {
    let bytes = physics_count_fixture()?;
    let model = read_bytes(bytes.clone(), LodSelection::All)?;
    check_physics_selection(&model)?;
    let [lod] = model.lods.as_slice() else {
        return Err("one CrCh level".into());
    };
    let [mesh] = lod.meshes.as_slice() else {
        return Err("only the nonempty physics chunk produces a mesh".into());
    };
    let source = mesh
        .crch_physics_source(&model.chunks)
        .ok_or("selected physics evidence")?;
    assert_eq!(
        source.chunk(),
        SELECTED_PHYSICS,
        "the empty earlier read is not selected"
    );
    let table = model
        .chunks
        .get(SELECTED_PHYSICS)
        .ok_or("selected physics table row")?;
    assert_eq!(
        source.triangle_count_range().start,
        table.layout.offset + TRIANGLE_COUNT_AT as u64,
        "original word address in the selected primary model",
    );
    assert_eq!(
        word(&bytes, source.primitive_range())?,
        1,
        "selected primitive"
    );
    for (range, count) in [
        (source.vertex_count_range(), mesh.positions.len()),
        (source.triangle_count_range(), lod.triangle_count()),
    ] {
        assert_eq!(
            usize::try_from(word(&bytes, range)?)?,
            count,
            "emitted count"
        );
    }

    let mut changed = mesh.clone();
    changed.indices.clear();
    assert!(
        changed.crch_physics_source(&model.chunks).is_none(),
        "transformed counts cannot reuse the proof"
    );
    assert!(
        mesh.crch_physics_source(&[]).is_none(),
        "the physical table binding is required"
    );
    Ok(())
}

fn check_physics_selection(model: &Model) -> TestResult {
    const EMPTY_PHYSICS: usize = 7;
    const SKIPPED_PHYSICS: usize = 9;
    for (ordinal, expected) in [
        (SELECTED_PHYSICS, ChunkUsage::Read),
        (EMPTY_PHYSICS, ChunkUsage::Read),
        (SKIPPED_PHYSICS, ChunkUsage::Skipped),
    ] {
        let usage = model.chunks.get(ordinal).ok_or("physical table row")?.usage;
        assert_eq!(usage, expected, "original physics chunk at {ordinal}");
    }
    Ok(())
}

const SELECTED_PHYSICS: usize = 3;
const TRIANGLE_COUNT_AT: usize = 100;

fn physics_count_fixture() -> Result<Vec<u8>, Box<dyn Error>> {
    const ROOT_NODE: usize = 2;
    const SELECTED_MESH: usize = 4;
    const EMPTY_PHYSICS_ID: u32 = 8;
    const EMPTY_MESH_ID: u32 = 9;
    const OBJECT_ID_AT: usize = 64;
    const FIRST_PHYSICS_SLOT_AT: usize = 92;

    let cube = ChunkFile::parse(TestCrchModel::cube().bytes())?;
    let physical_mesh = cube.chunk_data(SELECTED_MESH).ok_or("Mesh source")?;
    let physical_counts = cube
        .chunk_data(SELECTED_PHYSICS)
        .ok_or("MeshPhysicsData source")?;
    let mut zero = physical_counts.to_vec();
    set_word(&mut zero, TRIANGLE_COUNT_AT, 0)?;
    let mut zero_mesh = physical_mesh.to_vec();
    set_word(&mut zero_mesh, FIRST_PHYSICS_SLOT_AT, EMPTY_PHYSICS_ID)?;
    let mut fixture = TestChunkFile::crch();
    for (ordinal, chunk) in cube.chunks().iter().enumerate() {
        let mut bytes = cube.chunk_data(ordinal).ok_or("chunk payload")?.to_vec();
        if ordinal == ROOT_NODE {
            set_word(&mut bytes, OBJECT_ID_AT, EMPTY_MESH_ID)?;
        }
        fixture = fixture.chunk(chunk.chunk_type().id(), chunk.version(), bytes);
    }
    Ok(fixture
        .chunk(0x1018, 0x800, zero)
        .chunk(0x1000, 0x801, zero_mesh)
        .chunk(0x1018, 0x800, physical_counts.to_vec())
        .bytes())
}

fn set_word(bytes: &mut [u8], start: usize, word: u32) -> TestResult {
    bytes
        .get_mut(start..start + size_of::<u32>())
        .ok_or("fixture word address")?
        .copy_from_slice(&word.to_le_bytes());
    Ok(())
}

fn word(bytes: &[u8], range: std::ops::Range<u64>) -> Result<u32, Box<dyn Error>> {
    let range = usize::try_from(range.start)?..usize::try_from(range.end)?;
    let original = bytes.get(range).ok_or("original word address")?;
    Ok(u32::from_le_bytes(original.try_into()?))
}
