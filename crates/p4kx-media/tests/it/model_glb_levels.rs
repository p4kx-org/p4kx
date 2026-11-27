// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;
use std::{env, fs, io};

use p4kx_extraction::model::{LodSelection, Model, ModelFile, ModelIssue};
use p4kx_extraction::{GlbOptions, ModelSource, PathIndex};
use p4kx_gltf::{MeshVertexMap, PrimitiveMaterialSource, write_glb, write_glb_observed};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::gltf::Semantic;
use p4kx_testkit::{CheckedGlb, TestModel, TestSkeleton, check_glb};
use serde_json::json;

use super::model_glb_fixture::{Fixture, PRIMARY};
use super::model_glb_media::{check_field, check_metadata, original_vertex_order, physical_row};
use super::support::{self, TestResult};

#[test]
fn model_media_and_glb_observe_the_same_rigged_source_levels() -> TestResult {
    let fixture = Fixture::new();
    let directory = tempfile::tempdir()?;
    let archive = fixture.archive(directory.path())?;
    let output = directory.path().join("media");
    let options = ExportOptions {
        kinds: vec![MediaKind::Model],
        ..ExportOptions::default()
    };
    let media = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &media, "model.jsonl")?;
    check_metadata(&rows)?;
    let generation = output.join(&media.artifact_base);
    let base_order = original_vertex_order(&generation, &rows, 1, 2)?;
    let lod_order = original_vertex_order(&generation, &rows, 3, 4)?;
    assert_ne!(
        base_order, lod_order,
        "each physical geometry exercises its own index order"
    );
    let index = PathIndex::new(&archive);
    let model = ModelSource::from_index(&index, PRIMARY)?;
    let fallback = select_fallback(&index)?;
    let base = PhysicalLod {
        owner: 1,
        level: 0,
        file: ModelFile::Geometry,
        order: &base_order,
        fixture: &fixture.base,
    };
    let level_one = PhysicalLod {
        owner: 3,
        level: 1,
        file: ModelFile::LodGeometry(1),
        order: &lod_order,
        fixture: &fixture.level_one,
    };
    for (requested, selected, model) in [
        (0, &base, model.model()),
        (1, &level_one, model.model()),
        (3, &base, fallback.model()),
    ] {
        let source = Reconciliation {
            requested,
            selected,
            rows: &rows,
        };
        let bytes = source.export(model)?;
        source.check_glb(&bytes)?;
        if let Some(destination) = env::var_os("P4KX_SYNTHETIC_MODEL_GLB_DIR") {
            fs::write(
                Path::new(&destination).join(format!("lod{}.glb", source.requested)),
                bytes,
            )?;
        }
    }
    Ok(())
}

fn select_fallback(index: &PathIndex<'_>) -> Result<ModelSource, Box<dyn Error>> {
    let fallback = ModelSource::from_index_with(index, PRIMARY, LodSelection::OnlyOrBase(3))?;
    assert!(
        fallback
            .model()
            .issues
            .iter()
            .any(|issue| matches!(issue, ModelIssue::LodFallback { level: 3, .. })),
        "unavailable LOD is explicitly a fallback",
    );
    Ok(fallback)
}

struct PhysicalLod<'a> {
    owner: usize,
    level: u32,
    file: ModelFile,
    order: &'a [u32],
    fixture: &'a TestModel,
}

struct Reconciliation<'a> {
    requested: u32,
    selected: &'a PhysicalLod<'a>,
    rows: &'a [MediaRecord],
}

impl Reconciliation<'_> {
    fn export(&self, model: &Model) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut observed = Vec::new();
        let options = GlbOptions::default().with_lod(self.requested);
        let summary = write_glb_observed(
            model,
            &[],
            &options,
            None,
            None,
            &mut observed,
            &mut |map| {
                self.check_map(map)
                    .map_err(|error| io::Error::other(error.to_string()))
            },
        )?;
        let mut ordinary = Vec::new();
        write_glb(model, &options, &mut ordinary)?;
        assert_eq!(
            observed, ordinary,
            "ordinary writer and receipt emit identical GLB bytes"
        );
        assert_eq!(
            (summary.meshes, summary.skinned_meshes, summary.primitives),
            (1, 1, 2),
            "one skin with two drawn source rows"
        );
        Ok(observed)
    }

    fn check_map(&self, map: MeshVertexMap<'_>) -> TestResult {
        assert_eq!(
            (map.mesh, map.model_node, map.requested_lod, map.model_lod),
            (0, 1, self.requested, self.requested),
            "emitted node and selected model level"
        );
        assert_eq!(
            map.origin
                .map(|origin| (origin.file, origin.chunk, origin.lod)),
            Some((self.selected.file, 0, self.selected.level)),
            "actual original geometry file and level"
        );
        assert_eq!(
            map.source_vertices,
            Some(self.selected.order),
            "original source vertex ordinal for every GLB POSITION"
        );
        self.check_primitives(map)
    }

    fn check_primitives(&self, map: MeshVertexMap<'_>) -> TestResult {
        for (primitive, (ordinal, material, first, count)) in
            map.primitives().zip([(0, 1, 0, 12), (1, 2, 12, 24)])
        {
            let original = primitive.original.ok_or("positioned primitive source")?;
            let (_, row) = physical_row(
                self.rows,
                self.selected.owner,
                "model-mesh-submesh",
                &[0, u64::try_from(ordinal)?],
            )?;
            assert_eq!(primitive.primitive, ordinal, "actual GLB primitive ordinal");
            assert_eq!(
                primitive.material_index,
                Some(ordinal),
                "dense GLB material slot"
            );
            assert_eq!(
                primitive.material_source,
                PrimitiveMaterialSource::Library(material),
                "nondefault physical material slot"
            );
            assert_eq!(
                (
                    original.ordinal(),
                    original.node(),
                    usize::from(original.material()),
                    original.first_index(),
                    original.index_count()
                ),
                (ordinal, 1, material, first, count),
                "original physical submesh"
            );
            check_field(row, "node_id", &json!(original.node()));
            check_field(row, "material_id", &json!(original.material()));
            check_field(row, "index_count", &json!(original.index_count()));
        }
        Ok(())
    }

    fn check_glb(&self, bytes: &[u8]) -> TestResult {
        let glb = check_glb(bytes)?;
        let skeleton = if self.selected.level == 0 {
            TestSkeleton::two_bones()
        } else {
            TestSkeleton::chain(6)
        };
        check_skin(&glb, &skeleton)?;
        self.selected.check_written_positions(&glb)
    }
}

impl PhysicalLod<'_> {
    fn check_written_positions(&self, glb: &CheckedGlb) -> TestResult {
        let mesh = glb.document.meshes().next().ok_or("one emitted mesh")?;
        let positions = self.fixture_positions()?;
        for (primitive, name) in mesh.primitives().zip(["caps_fixture", "sides_fixture"]) {
            assert_eq!(
                primitive.material().name(),
                Some(name),
                "written primitive material"
            );
            let accessor = primitive
                .get(&Semantic::Positions)
                .ok_or("shared POSITION accessor")?;
            let actual = glb.floats(&accessor);
            assert!(
                actual
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|position| position.map(f32::to_bits))
                    .eq(positions.iter().copied()),
                "physical index order and written positions agree after axis conversion",
            );
        }
        Ok(())
    }

    fn fixture_positions(&self) -> Result<Vec<[u32; 3]>, Box<dyn Error>> {
        self.order
            .iter()
            .map(|&source| {
                let index = usize::try_from(source)?;
                let [x, y, z] = self
                    .fixture
                    .vertices()
                    .get(index)
                    .ok_or("authored vertex")?
                    .position;
                Ok([x, z, -y].map(f32::to_bits))
            })
            .collect()
    }
}

fn check_skin(glb: &CheckedGlb, skeleton: &TestSkeleton) -> TestResult {
    let skin = glb.document.skins().next().ok_or("one emitted skin")?;
    assert_eq!(
        skin.joints().len(),
        skeleton.bones().len(),
        "joint count of source level"
    );
    assert!(
        skin.joints()
            .zip(skeleton.bones())
            .all(|(node, bone)| node.name() == Some(bone.name.as_str())),
        "joint names and order of the selected source level",
    );
    let binds = skin
        .inverse_bind_matrices()
        .ok_or("ordered inverse bind matrices")?;
    check_binds(&glb.floats(&binds), skeleton);
    Ok(())
}

fn check_binds(binds: &[f32], skeleton: &TestSkeleton) {
    let (matrices, remainder) = binds.as_chunks::<16>();
    assert_eq!(
        (matrices.len(), remainder.len()),
        (skeleton.bones().len(), 0),
        "one ordered 4x4 inverse bind for each authored joint"
    );
    for (bind, (_, [x, y, z])) in matrices.iter().zip(skeleton.world()) {
        let p = [x, z, -y, 1.0];
        let column_major = bind.as_chunks::<4>().0;
        for (row, expected) in [0.0_f32, 0.0, 0.0, 1.0].into_iter().enumerate() {
            let mapped: f32 = column_major
                .iter()
                .zip(p)
                .filter_map(|(column, component)| column.get(row).map(|value| component * value))
                .sum();
            assert!(
                (mapped - expected).abs() < 1e-5,
                "inverse bind maps original joint origin to zero, component {row}"
            );
        }
    }
}
