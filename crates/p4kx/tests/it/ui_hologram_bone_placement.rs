// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ops::Range;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_testkit::{
    DataForgeBuilder, TestBone, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestModel,
    TestMtl, TestMtlMaterial, TestNode, TestPort, TestSkeleton, entity_schema, test_key,
};
use serde_json::{Value, json};

use super::{TestResult, hologram, receipt, skip, source_uses, write_archive};

const SHIP: &str = "Data\\Objects\\Test\\ship.cgf";
const LOD: &str = "Data\\Objects\\Test\\ship_lod1.cgf";
const CHILD: &str = "Data\\Objects\\Test\\child.cgf";
const BONE: &str = "modelBonePlacementBytes";
const NAME_TABLE: &[u8] = b"base\0Socket\0SOCKET\0unused\0";

fn skeleton(x: f32) -> TestSkeleton {
    let bone = |name: &str, parent, translation| TestBone {
        name: name.to_owned(),
        parent,
        rotation: [0.0, 0.0, 0.0, 1.0],
        translation,
    };
    TestSkeleton::new(vec![
        bone("base", None, [2.0, 0.0, 0.0]),
        bone("Socket", Some(0), [x, 3.0, 0.0]),
        bone("SOCKET", Some(0), [90.0, 0.0, 0.0]),
        bone("unused", None, [70.0, 0.0, 0.0]),
    ])
}

fn node(name: &str, parent: Option<u16>, kind: u16, x: f32) -> TestNode {
    TestNode {
        name: name.to_owned(),
        parent,
        local: [
            [1.0, 0.0, 0.0, x],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        kind,
    }
}

fn hull(with_node: bool) -> TestModel {
    let weighted = || TestModel::cube().with_bone_map(false, vec![vec![(0, 255)]; 24]);
    let mut nodes = vec![node("hull", None, 2, 0.0), node("mesh", Some(0), 0, 0.0)];
    if with_node {
        nodes.push(node("socket", Some(0), 2, 25.0));
    }
    weighted()
        .with_nodes(nodes)
        .with_material_library("objects/test/hidden")
        .with_skeleton(&skeleton(4.0))
        .with_lod(weighted().with_compact_skeleton(&skeleton(8.0)))
}

fn forge() -> Vec<u8> {
    let root = TestEntity::new(1, "TEST_Ship", "entities/spaceships/test_ship.xml")
        .geometry("Objects/Test/ship.cgf", None)
        .port(TestPort::new("mount").helper("socket", [0.0; 3], [0.0; 3]))
        .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
            "mount",
            "TEST_Child",
        )]));
    let child = TestEntity::new(2, "TEST_Child", "entities/scitem/test_child.xml")
        .geometry("Objects/Test/child.cgf", None);
    entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(child.record())
        .build()
}

fn fixture(model: &TestModel) -> Result<Vec<TestEntry>, Box<dyn Error>> {
    let hidden = || TestMtlMaterial::new("hidden").shader("NoDraw");
    let library = TestMtl::multi(&[hidden(), hidden(), hidden()]).cryxml();
    Ok(vec![
        TestEntry::zstd("Data\\Game2.dcb", forge()),
        TestEntry::stored(SHIP, model.cgf()),
        TestEntry::stored("Data\\Objects\\Test\\ship.cgfm", model.cgfm()),
        TestEntry::stored(LOD, model.lod_cgf(1).ok_or("LOD model")?).encrypted(),
        TestEntry::stored(
            "Data\\Objects\\Test\\ship_lod1.cgfm",
            model.lod_cgfm(1).ok_or("LOD geometry")?,
        ),
        TestEntry::stored(CHILD, TestModel::cube().single_file().scaled(0.4).cgf()),
        TestEntry::stored("Data\\Objects\\Test\\hidden.mtl", library),
    ])
}

#[test]
fn port_bone_origin_uses_the_effective_skeleton_and_loses_to_a_named_node() -> TestResult {
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("ship.png");
    let key = test_key().to_hex();
    for (lod, with_node) in [("0", false), ("1", false), ("1", true)] {
        let model = hull(with_node);
        let input = tempfile::tempdir_in(dir.path())?;
        let archive = write_archive(input.path(), fixture(&model)?)?;
        let run = hologram(
            &archive,
            &out,
            &["--size", "96", "--lod", lod, "--key", &key],
            true,
        )?;
        if skip(
            "port_bone_origin_uses_the_effective_skeleton_and_loses_to_a_named_node",
            &run,
        ) {
            return Ok(());
        }
        assert_eq!(run.code, Some(0), "{}", run.stderr);
        let report: Value = serde_json::from_str(&run.stdout)?;
        let rows = receipt(&out, &report)?;
        assert_eq!(
            report["drawn_parts"], 1,
            "only the attached child's mesh is drawn"
        );
        let ship = source_uses(&rows, &report, "ship.cgf");
        let level = source_uses(&rows, &report, "ship_lod1.cgf");
        if with_node {
            assert_eq!(
                level,
                [] as [&Value; 0],
                "the encrypted LOD skeleton was bypassed"
            );
            assert!(
                ship.iter().any(
                    |row| row.pointer("/location/namespace").and_then(Value::as_str)
                        == Some("modelNodePlacementBytes")
                ),
                "the helper node actually won"
            );
        } else if lod == "1" {
            assert_eq!(
                ship,
                [] as [&Value; 0],
                "the base model supplied no rendered datum"
            );
            check_bones(&model.lod_cgf(1).ok_or("LOD skeleton")?, &level, true)?;
        } else {
            assert_eq!(level, [] as [&Value; 0], "unselected LOD skeleton");
            check_bones(&model.cgf(), &ship, false)?;
        }
        assert_eq!(
            report["artifactOrigin"]["containsEncryptedSource"],
            json!((lod == "1" && !with_node).then_some(true))
        );
    }
    Ok(())
}

fn check_bones(bytes: &[u8], uses: &[&Value], compact: bool) -> TestResult {
    let file = ChunkFile::parse(bytes)?;
    let (kind, records_at, parent_width, stride) = if compact {
        (ivo::COMPILED_BONES, 48, 2, 16)
    } else {
        (ivo::COMPILED_BONES_2, 4, 4, 68)
    };
    let (chunk, row) = file.chunks_of(kind).next().ok_or("skeleton chunk")?;
    let records = row.offset() + records_at;
    let names = records + 4 * stride;
    assert_eq!(
        uses.len(),
        5,
        "only chosen name, parent links and relative poses"
    );
    assert_site(uses, chunk, 1, "name", names + 5..names + 12, "selector");
    for bone in [0, 1] {
        let parent = records + bone * stride + 8;
        let pose = if compact {
            names + NAME_TABLE.len() as u64 + bone * 28
        } else {
            parent + 4
        };
        assert_site(
            uses,
            chunk,
            bone,
            "parent",
            parent..parent + parent_width,
            "selector",
        );
        assert_site(
            uses,
            chunk,
            bone,
            "relativePose",
            pose..pose + 28,
            "conversion",
        );
    }
    Ok(())
}

fn assert_site(
    uses: &[&Value],
    chunk: usize,
    bone: u64,
    field: &str,
    range: Range<u64>,
    role: &str,
) {
    let location = json!({
        "type": "subresource",
        "namespace": BONE,
        "field": field,
        "indices": [chunk.to_string(), bone.to_string(), range.start.to_string(), range.end.to_string()],
    });
    assert!(
        uses.iter().any(|row| row.get("location") == Some(&location)
            && row.get("role").and_then(Value::as_str) == Some(role)),
        "missing {location} as {role}: {uses:?}",
    );
}
