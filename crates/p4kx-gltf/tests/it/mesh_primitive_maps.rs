// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_gltf::{
    GlbOptions, LoadedTexture, MeshPrimitiveMap, PrimitiveMaterialSource, RigPiece,
    TextureLoadError, TextureLoader, write_glb, write_glb_observed, write_rigged_glb_animated_with,
    write_rigged_glb_with_vertex_maps,
};
use p4kx_model::{CompanionError, CompanionPath, Material, Model};
use p4kx_testkit::{
    CUBE_MATERIAL_LIBRARY, TestChunkFile, TestModel, TestMtl, TestMtlMaterial, TestNode,
    TestSubmesh, check_glb,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn hidden_and_empty_submeshes_do_not_shift_authored_primitive_owners() -> TestResult {
    let fixture = Fixture::new(0x900);
    let model = fixture.model()?;
    let mut ordinary = Vec::new();
    write_glb(&model, &GlbOptions::default(), &mut ordinary)?;
    let (glb, rows) = observe(&model)?;
    assert_eq!(glb, ordinary, "extra observation does not change GLB bytes");
    fixture.assert_output(&glb, &rows, PrimitiveMaterialSource::BuiltInFallback)
}

#[test]
fn rigged_material_slots_use_the_actual_supplied_fallback_and_source_rows() -> TestResult {
    let fixture = Fixture::new(0x900);
    let model = fixture.model()?;
    let fallback = Material::engine_default();
    let pieces = [RigPiece::new("rig", &model, &model.materials).with_fallback(&fallback)];
    let mut rows = Vec::new();
    let mut glb = Vec::new();
    let options = GlbOptions::default();
    let textures = NoTextures;
    write_rigged_glb_with_vertex_maps(
        &model.bones,
        &pieces,
        &[],
        &options,
        &textures,
        &mut glb,
        &mut |mesh| {
            rows.extend(mesh.primitives());
            Ok(())
        },
    )?;
    let mut ordinary = Vec::new();
    write_rigged_glb_animated_with(
        &model.bones,
        &pieces,
        &[],
        &options,
        &textures,
        &mut ordinary,
    )?;
    assert_eq!(
        glb, ordinary,
        "rigged primitive capture does not change GLB bytes"
    );
    fixture.assert_output(&glb, &rows, PrimitiveMaterialSource::SuppliedFallback)
}

#[test]
fn unmeasured_chunk_version_never_claims_a_physical_submesh_row() -> TestResult {
    let fixture = Fixture::new(0x901);
    let (_, rows) = observe(&fixture.model()?)?;
    assert_eq!(
        rows.len(),
        4,
        "ordinary GLB primitive structure is still readable"
    );
    assert!(
        rows.iter().all(|row| row.original.is_none()),
        "unmeasured version remains unproved"
    );
    Ok(())
}

#[test]
fn static_node_grouping_keeps_each_physical_submesh_ordinal() -> TestResult {
    let row = |node, first_index| TestSubmesh {
        material_id: 0,
        node,
        first_index,
        index_count: 3,
        first_vertex: 0,
        vertex_count: 24,
    };
    let nodes = ["first", "second"]
        .into_iter()
        .map(|name| TestNode {
            name: name.to_owned(),
            parent: None,
            local: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
            kind: 0,
        })
        .collect();
    let model = TestModel::cube().with_nodes(nodes).with_submeshes(vec![
        row(1, 0),
        row(0, 3),
        row(1, 6),
        row(0, 9),
    ]);
    let fixture = Fixture {
        primary: model.cgf(),
        companion: model.cgfm(),
        library: TestMtl::single(&TestMtlMaterial::new("material")).cryxml(),
    };
    let (_, rows) = observe(&fixture.read("sample.cgf", "sample.cgfm")?)?;
    let actual = rows
        .iter()
        .map(|row| row.original.map(|source| (source.ordinal(), source.node())))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [Some((1, 0)), Some((3, 0)), Some((0, 1)), Some((2, 1))],
        "physical rows remain traceable after sorting by the static node"
    );
    Ok(())
}

fn observe(model: &Model) -> TestResult<(Vec<u8>, Vec<MeshPrimitiveMap>)> {
    let mut written = Vec::new();
    let mut rows = Vec::new();
    write_glb_observed(
        model,
        &[],
        &GlbOptions::default(),
        None,
        None,
        &mut written,
        &mut |mesh| {
            rows.extend(mesh.primitives());
            Ok(())
        },
    )?;
    Ok((written, rows))
}

struct NoTextures;

impl TextureLoader for NoTextures {
    fn load(&self, _: &str, _: u32) -> Result<LoadedTexture, TextureLoadError> {
        Err(TextureLoadError::NotFound)
    }
}

struct Fixture {
    primary: Vec<u8>,
    companion: Vec<u8>,
    library: Vec<u8>,
}

impl Fixture {
    fn new(version: u32) -> Self {
        let model = TestModel::two_bone_skin().with_submeshes(
            [
                (2, 11, 0, 3),
                (0, 12, 3, 0),
                (4, 21, 3, 3),
                (4, 22, 6, 3),
                (10, 23, 9, 3),
                (14, 24, 12, 3),
            ]
            .into_iter()
            .map(
                |(material_id, node, first_index, index_count)| TestSubmesh {
                    material_id,
                    node,
                    first_index,
                    index_count,
                    first_vertex: 0,
                    vertex_count: 24,
                },
            )
            .collect(),
        );
        let library = TestMtl::multi(&[
            TestMtlMaterial::new("unused"),
            TestMtlMaterial::new("equal name"),
            TestMtlMaterial::new("hidden").shader("NoDraw"),
            TestMtlMaterial::new("unused"),
            TestMtlMaterial::new("equal name"),
        ])
        .cryxml();
        let companion = TestChunkFile::ivo()
            .chunk(0xDEAD_CAFE, 0x900, [0; 8])
            .chunk(ivo::SKIN_2.id(), version, model.skin_chunk())
            .bytes();
        Self {
            primary: model.cgf(),
            companion,
            library,
        }
    }

    fn model(&self) -> TestResult<Model> {
        self.read("sample.skin", "sample.skinm")
    }

    fn read(&self, name: &str, geometry: &str) -> TestResult<Model> {
        let model = ChunkFile::parse(self.primary.as_slice())?;
        let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
            Ok(match path {
                CompanionPath::Sibling(path) if path == geometry => Some(self.companion.clone()),
                CompanionPath::Material(CUBE_MATERIAL_LIBRARY) => Some(self.library.clone()),
                _ => None,
            })
        };
        Ok(Model::read(name, &model, &mut companions)?)
    }

    fn assert_output(
        &self,
        glb: &[u8],
        rows: &[MeshPrimitiveMap],
        fallback: PrimitiveMaterialSource,
    ) -> TestResult {
        let parsed = check_glb(glb)?;
        let mesh = parsed.document.meshes().next().ok_or("output mesh")?;
        assert_eq!(rows.len(), 4, "hidden and empty physical rows excluded");
        assert_eq!(
            mesh.primitives().count(),
            rows.len(),
            "all actual output primitives observed"
        );
        let original = ChunkFile::parse(self.companion.as_slice())?;
        let (_, chunk) = original
            .chunks_of(ivo::SKIN_2)
            .next()
            .ok_or("actual mesh chunk")?;
        let expected = [
            (2, 4, 21, 0, PrimitiveMaterialSource::Library(4)),
            (3, 4, 22, 0, PrimitiveMaterialSource::Library(4)),
            (4, 10, 23, 1, fallback),
            (5, 14, 24, 1, fallback),
        ];
        for ((primitive, row), (ordinal, id, zone, dense, source)) in
            mesh.primitives().zip(rows).zip(expected)
        {
            assert_eq!(
                row.primitive,
                primitive.index(),
                "actual emitted primitive index"
            );
            let accessor = primitive.indices().ok_or("emitted index accessor")?;
            assert_eq!(
                row.index_accessor,
                Some(accessor.index()),
                "actual GLB index accessor"
            );
            assert_eq!(
                row.material_index,
                primitive.material().index(),
                "actual GLB material index"
            );
            assert_eq!(
                row.material_index,
                Some(dense),
                "material sharing leaves actual dense assignment"
            );
            assert_eq!(row.material_source, source, "original material disposition");
            assert_eq!(
                row.model_submesh,
                ordinal - 1,
                "one empty source row removed before model grouping"
            );
            assert_original(row, ordinal, id, zone, chunk.offset())?;
        }
        Ok(())
    }
}

fn assert_original(
    row: &MeshPrimitiveMap,
    ordinal: usize,
    id: u16,
    zone: u16,
    chunk_offset: u64,
) -> TestResult {
    let original = row.original.ok_or("original bounded submesh row")?;
    assert_eq!(
        original.ordinal(),
        ordinal,
        "physical ordinal before grouping/filtering"
    );
    assert_eq!(original.material(), id, "raw material word");
    assert_eq!(
        original.node(),
        zone,
        "skin zone survives collapsed render node"
    );
    let start = chunk_offset + 168 + ordinal as u64 * 48;
    assert_eq!(
        original.range(),
        start..start + 48,
        "exact geometry companion row"
    );
    assert_eq!(
        original.material_range(),
        start..start + 2,
        "direct material byte span"
    );
    assert_eq!(
        original.node_range(),
        start + 2..start + 4,
        "direct skin-zone byte span"
    );
    Ok(())
}
