// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io;

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_gltf::{
    GlbOptions, MeshVertexMap, Scene, SceneNode, ScenePart, write_glb, write_glb_observed,
    write_scene_glb_observed,
};
use p4kx_model::{CompanionError, CompanionPath, LodSelection, Model, ModelFile, Transform};
use p4kx_testkit::gltf::Semantic;
use p4kx_testkit::{
    CUBE_MATERIAL_LIBRARY, TestChunkFile, TestModel, TestMtl, TestMtlMaterial, TestPositions,
    check_glb, mesh_streams,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const INDEX_TAG: u32 = 0xEECD_C168;
const TRIANGLE_AT: usize = 168;

struct Fixture {
    primary: Vec<u8>,
    base: Vec<u8>,
    level_one: Vec<u8>,
    base_mesh: Vec<u8>,
    level_one_mesh: Vec<u8>,
    material: Vec<u8>,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let source = TestModel::cube().with_positions(TestPositions::Float);
        let base_mesh = source.skin_chunk();
        let mut level_one_mesh = source.scaled(0.5).with_wide_indices().skin_chunk();
        let row = level_one_mesh
            .get_mut(TRIANGLE_AT..TRIANGLE_AT + 48)
            .ok_or("first row")?;
        let words = row.get_mut(4..20).ok_or("first submesh words")?;
        for (slot, value) in words
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip([0_u32, 3, 0, 4])
        {
            *slot = value.to_le_bytes();
        }
        let level_one = TestChunkFile::ivo()
            .chunk(0xDEAD_CAFE, 0x900, [0; 8])
            .chunk(ivo::SKIN_2.id(), 0x900, level_one_mesh.as_slice())
            .bytes();
        let material = TestMtl::multi(&[
            TestMtlMaterial::new("unused"),
            TestMtlMaterial::new("caps"),
            TestMtlMaterial::new("sides").shader("NoDraw"),
        ])
        .cryxml();
        Ok(Self {
            primary: TestModel::cube().cgf(),
            base: TestChunkFile::ivo()
                .chunk(ivo::SKIN_2.id(), 0x900, base_mesh.as_slice())
                .bytes(),
            level_one,
            base_mesh,
            level_one_mesh,
            material,
        })
    }

    fn read(&self, lod: LodSelection) -> TestResult<Model> {
        let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
            Ok(match path {
                CompanionPath::Sibling("cube.cgfm") => Some(self.base.clone()),
                CompanionPath::Sibling("cube_lod1.cgfm") => Some(self.level_one.clone()),
                CompanionPath::Material(CUBE_MATERIAL_LIBRARY) => Some(self.material.clone()),
                _ => None,
            })
        };
        Ok(Model::read_with(
            "cube.cgf",
            &ChunkFile::parse(self.primary.as_slice())?,
            &mut companions,
            lod,
        )?)
    }
}

#[test]
fn each_written_level_maps_to_original_index_words_and_position_bytes() -> TestResult {
    let fixture = Fixture::new()?;
    let model = fixture.read(LodSelection::All)?;
    for (level, source, file, chunk, width) in [
        (0, fixture.base_mesh.as_slice(), ModelFile::Geometry, 0, 2),
        (
            1,
            fixture.level_one_mesh.as_slice(),
            ModelFile::LodGeometry(1),
            1,
            4,
        ),
    ] {
        let options = GlbOptions::default().with_lod(level);
        let mut observed = Vec::new();
        let mut baseline = Vec::new();
        let summary = write_glb_observed(
            &model,
            &[],
            &options,
            None,
            None,
            &mut observed,
            &mut |map| {
                assert_map(map, level, level, level, file, chunk);
                assert_vertices(map.source_vertices, source, width)?;
                Ok(())
            },
        )?;
        write_glb(&model, &options, &mut baseline)?;
        assert_eq!(observed, baseline, "observation never changes output");
        assert_eq!(summary.hidden_primitives, 1, "hidden sides are omitted");
        let expected = original_first_use(source, width)?;
        check_accessors(&observed, source, &expected)?;
    }
    Ok(())
}

fn assert_map(
    map: MeshVertexMap<'_>,
    requested: u32,
    model: u32,
    physical: u32,
    file: ModelFile,
    chunk: usize,
) {
    assert_eq!(
        (map.mesh, map.part, map.model_node),
        (0, 0, 1),
        "emitted mesh owner"
    );
    assert_eq!(
        (map.requested_lod, map.model_lod),
        (requested, model),
        "LOD selection"
    );
    assert_eq!(
        map.origin.map(|row| (row.file, row.chunk, row.lod)),
        Some((file, chunk, physical)),
        "physical IVO geometry"
    );
    assert_eq!(map.position_accessor, 0, "emitted POSITION accessor");
}

fn assert_vertices(found: Option<&[u32]>, mesh: &[u8], width: usize) -> io::Result<()> {
    let expected =
        original_first_use(mesh, width).map_err(|error| io::Error::other(error.to_string()))?;
    assert_eq!(
        found,
        Some(expected.as_slice()),
        "exact raw first-use order"
    );
    Ok(())
}

fn original_first_use(mesh: &[u8], width: usize) -> TestResult<Vec<u32>> {
    let streams = mesh_streams(mesh).ok_or("physical inventory")?;
    let range = streams
        .streams
        .iter()
        .find(|stream| stream.tag == INDEX_TAG)
        .ok_or("physical index stream")?;
    assert_eq!(range.element_size as usize, width, "source index width");
    let data = mesh.get(range.data.clone()).ok_or("physical index words")?;
    let mut order = Vec::new();
    for row in mesh
        .get(TRIANGLE_AT..TRIANGLE_AT + 96)
        .ok_or("authored rows")?
        .as_chunks::<48>()
        .0
    {
        let first = word(row, 4)? as usize;
        let count = word(row, 8)? as usize;
        let page = word(row, 16)?;
        for index in first..first + count {
            let start = index * width;
            let bytes = data
                .get(start..start + width)
                .ok_or("physical index word")?;
            let raw = match *bytes {
                [a, b] => u32::from(u16::from_le_bytes([a, b])),
                [a, b, c, d] => u32::from_le_bytes([a, b, c, d]),
                _ => return Err("source word width".into()),
            };
            let vertex = raw + page;
            if !order.contains(&vertex) {
                order.push(vertex);
            }
        }
    }
    Ok(order)
}

fn check_accessors(glb: &[u8], mesh: &[u8], order: &[u32]) -> TestResult {
    let glb = check_glb(glb)?;
    let primitive = glb
        .document
        .meshes()
        .next()
        .and_then(|mesh| mesh.primitives().next())
        .ok_or("emitted primitive")?;
    let accessor = primitive
        .get(&Semantic::Positions)
        .ok_or("POSITION accessor")?;
    let positions = mesh_streams(mesh)
        .ok_or("physical inventory")?
        .streams
        .into_iter()
        .find(|stream| stream.tag == 0x9132_9AE9)
        .ok_or("physical position stream")?;
    let positions = mesh.get(positions.data).ok_or("physical position words")?;
    let actual = glb.floats(&accessor);
    assert_eq!(
        actual.len(),
        order.len() * 3,
        "one POSITION per reported vertex"
    );
    for (actual, source) in actual.as_chunks::<3>().0.iter().zip(order) {
        let at = *source as usize * 20;
        let [x, y, z] = [
            word(positions, at)?,
            word(positions, at + 4)?,
            word(positions, at + 8)?,
        ]
        .map(f32::from_bits);
        assert_eq!(
            actual.map(f32::to_bits),
            [x, z, -y].map(f32::to_bits),
            "original float position after GLB axis change"
        );
    }
    Ok(())
}

fn word(data: &[u8], offset: usize) -> TestResult<u32> {
    let bytes = data.get(offset..offset + 4).ok_or("word bytes")?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
}

#[test]
fn scene_reuses_one_observed_mesh_and_keeps_requested_selected_and_physical_levels() -> TestResult {
    let fixture = Fixture::new()?;
    let model = fixture.read(LodSelection::All)?;
    let mut scene = Scene::new();
    scene.push(SceneNode::new("wrapper only", None, Transform::IDENTITY))?;
    for label in ["first", "twin"] {
        scene.push(
            SceneNode::new(label, None, Transform::IDENTITY)
                .with_part(ScenePart::new(&model, &model.materials)),
        )?;
    }
    let mut calls = 0;
    let summary = write_scene_glb_observed(
        &scene,
        &GlbOptions::default().with_lod(3),
        None,
        &mut Vec::new(),
        &mut |map| {
            calls += 1;
            assert_map(map, 3, 1, 1, ModelFile::LodGeometry(1), 1);
            assert_eq!(map.part, 0, "first producing model instance");
            assert_eq!(map.scene_node, Some(1), "producer's actual scene wrapper");
            Ok(())
        },
    )?;
    assert_eq!(
        (summary.meshes, summary.shared_meshes, calls),
        (1, 1, 1),
        "one output for twin instances"
    );

    let relabelled = fixture.read(LodSelection::OnlyOrBase(3))?;
    write_glb_observed(
        &relabelled,
        &[],
        &GlbOptions::default().with_lod(3),
        None,
        None,
        &mut Vec::new(),
        &mut |map| {
            assert_map(map, 3, 3, 0, ModelFile::Geometry, 0);
            Ok(())
        },
    )?;
    Ok(())
}
