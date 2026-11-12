// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::ops::Range;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestModel, TestMtl, TestMtlMaterial, TestNode,
    TestSubmesh, entity_schema,
};
use serde_json::{Value, json};

use super::{TestResult, hologram, receipt, skip, source_uses, write_archive};

const SHIP: &str = "Data\\Objects\\Test\\ship.cgf";
const MESH: &str = "modelMeshNodeBytes";

fn node(name: &str, parent: Option<u16>, scale: f32, shift: [f32; 3], kind: u16) -> TestNode {
    let [x, y, z] = shift;
    TestNode {
        name: name.to_owned(),
        parent,
        local: [
            [scale, 0.0, 0.0, x],
            [0.0, scale, 0.0, y],
            [0.0, 0.0, scale, z],
        ],
        kind,
    }
}

const fn mesh(first_index: u32, index_count: u32, node: u16, material_id: u16) -> TestSubmesh {
    TestSubmesh {
        material_id,
        node,
        first_index,
        index_count,
        first_vertex: 0,
        vertex_count: 24,
    }
}

fn model(interior_x: f32, outside_x: f32) -> Vec<u8> {
    let nested = Some(0);
    TestModel::cube()
        .single_file()
        .with_material_library("objects/test/ship")
        .with_nodes(vec![
            node("root", None, 1.0, [0.0; 3], 2),
            node("main", nested, 1.0, [0.0; 3], 0),
            node("inner drawn", nested, 0.25, [0.0, 0.0, 1.5], 0),
            node("inner hidden", nested, 0.25, [interior_x, 0.0, 1.5], 0),
            node("bound-only hidden", nested, 0.25, [outside_x, 0.0, 1.5], 0),
            node(
                "unused helper",
                nested,
                1.0,
                [interior_x * 1000.0, 0.0, 0.0],
                2,
            ),
        ])
        .with_submeshes(vec![
            mesh(0, 18, 1, 1),
            mesh(18, 6, 2, 1),
            mesh(24, 6, 3, 0),
            mesh(30, 6, 4, 0),
        ])
        .cgf()
}

fn fixture(model: Vec<u8>) -> Vec<TestEntry> {
    let root = TestEntity::new(1, "TEST_Ship", "entities/spaceships/test_ship.xml")
        .geometry("Objects/Test/ship.cgf", None);
    let forge = entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .build();
    let library = TestMtl::multi(&[
        TestMtlMaterial::new("hidden").shader("NoDraw"),
        TestMtlMaterial::new("visible").shader("Illum"),
    ])
    .cryxml();
    vec![
        TestEntry::zstd("Data\\Game2.dcb", forge),
        TestEntry::stored(SHIP, model),
        TestEntry::stored("Data\\Objects\\Test\\ship.mtl", library),
    ]
}

#[test]
fn only_the_visible_ranges_and_bounds_winners_use_direct_mesh_node_sources() -> TestResult {
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("ship.png");
    let mut original = None;
    for (inside, outside, changed) in [(0.0, 11.0, false), (0.25, 11.0, false), (0.0, 18.0, true)] {
        let input = tempfile::tempdir_in(dir.path())?;
        let bytes = model(inside, outside);
        let file = ChunkFile::parse(bytes.as_slice())?;
        let archive = write_archive(input.path(), fixture(bytes.clone()))?;
        let run = hologram(&archive, &out, &["--size", "128"], true)?;
        if skip(
            "only_the_visible_ranges_and_bounds_winners_use_direct_mesh_node_sources",
            &run,
        ) {
            return Ok(());
        }
        assert_eq!(run.code, Some(0), "{}", run.stderr);
        let report: Value = serde_json::from_str(&run.stdout)?;
        let rows = receipt(&out, &report)?;
        check_uses(&file, &rows, &report)?;
        let png = fs::read(&out)?;
        if let Some(original) = original.as_ref() {
            assert_eq!(
                *original != png,
                changed,
                "only an effective bound mutation changes the PNG"
            );
        } else {
            original = Some(png);
        }
    }
    Ok(())
}

fn check_uses(file: &ChunkFile<&[u8]>, rows: &[Value], report: &Value) -> TestResult {
    let uses = source_uses(rows, report, "ship.cgf");
    let uses: Vec<_> = uses
        .into_iter()
        .filter(|row| row.pointer("/location/namespace").and_then(Value::as_str) == Some(MESH))
        .collect();
    assert_eq!(
        uses.len(),
        12,
        "three effective mesh routes with two fields for each of two nodes"
    );
    let (chunk, row) = file
        .chunks_of(ivo::NODE_MESH_COMBO)
        .next()
        .ok_or("original node chunk")?;
    let root = row.offset() + 32;
    let scene = report
        .pointer("/fieldOrigins/~1drawn_parts/sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("scene source refs")?;
    for (mesh, selected, bound) in [(0, 1, true), (1, 2, false), (3, 4, true)] {
        for node in [0, selected] {
            let record = root + node * 208;
            let matrix = site(
                &uses,
                [mesh, u64::try_from(chunk)?, node],
                "localTransform",
                record + 80..record + 128,
                "conversion",
            )?;
            let parent = site(
                &uses,
                [mesh, u64::try_from(chunk)?, node],
                "parent",
                record + 148..record + 150,
                "selector",
            )?;
            assert_eq!(
                scene.contains(matrix),
                bound,
                "bounds vs visible-only matrix"
            );
            assert_eq!(
                scene.contains(parent),
                bound,
                "bounds vs visible-only parent"
            );
        }
    }
    Ok(())
}

fn site<'a>(
    uses: &[&'a Value],
    [mesh, chunk, node]: [u64; 3],
    field: &str,
    range: Range<u64>,
    role: &str,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    let indices = [0, mesh, chunk, node, range.start, range.end].map(|index| index.to_string());
    let location = json!({
        "type": "subresource", "namespace": MESH, "field": field,
        "indices": indices,
    });
    uses.iter()
        .find(|row| {
            row.get("location") == Some(&location)
                && row.get("role").and_then(Value::as_str) == Some(role)
        })
        .and_then(|row| row.get("id"))
        .ok_or_else(|| format!("missing source site {location} as {role}").into())
}
