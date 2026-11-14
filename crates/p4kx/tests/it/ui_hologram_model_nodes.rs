// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::ops::Range;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_testkit::{
    DataForgeBuilder, TestCrchModel, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry,
    TestModel, TestNode, TestPort, entity_schema, hologram_suppression_entries, test_key,
};
use serde_json::{Value, json};

use super::{TestResult, hologram, receipt, skip, source_uses, write_archive};

const SHIP: &str = "Data\\Objects\\Test\\ship.cgf";
const CHILD: &str = "Data\\Objects\\Test\\child.cgf";
const PLACEMENT: &str = "modelNodePlacementBytes";

fn model_node(name: &str, parent: Option<u16>, x: f32, kind: u16) -> TestNode {
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

fn hull(anchor: f32, losing: f32) -> Vec<u8> {
    TestModel::cube()
        .with_material_library("objects/test/ship")
        .with_nodes(vec![
            model_node("ship", None, 0.0, 2),
            model_node("ship_mesh", Some(0), 0.0, 0),
            model_node("hp_nose", Some(0), anchor, 2),
            model_node("hardpoint_relay", Some(0), 3.0, 2),
            model_node("HP_NOSE", Some(0), losing, 2),
            model_node("hardpoint_weapon_nose", Some(0), losing, 2),
            model_node("hardpoint_seat", Some(0), losing, 2),
        ])
        .cgf()
}

fn hull_entries(bytes: Vec<u8>, encrypted: bool) -> Vec<TestEntry> {
    let mut entries = hologram_suppression_entries();
    entries.retain(|entry| entry.path() != SHIP);
    let model = TestEntry::stored(SHIP, bytes);
    entries.push(if encrypted { model.encrypted() } else { model });
    entries
}

#[test]
fn only_used_model_nodes_supply_transforms_even_when_their_mesh_is_suppressed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let key = test_key().to_hex();
    let out = dir.path().join("ship.png");
    let mut baseline = None;
    for (encrypted, anchor, losing, picture_changed) in [
        (true, 4.0, 20.0, false),
        (false, 4.0, -300.0, false),
        (false, -3.0, 20.0, true),
    ] {
        let input = tempfile::tempdir_in(dir.path())?;
        let bytes = hull(anchor, losing);
        let file = ChunkFile::parse(bytes.as_slice())?;
        let archive = write_archive(input.path(), hull_entries(bytes.clone(), encrypted))?;
        let run = hologram(&archive, &out, &["--size", "96", "--key", &key], true)?;
        if skip(
            "only_used_model_nodes_supply_transforms_even_when_their_mesh_is_suppressed",
            &run,
        ) {
            return Ok(());
        }
        assert_eq!(run.code, Some(0), "{}", run.stderr);
        let report: Value = serde_json::from_str(&run.stdout)?;
        let rows = receipt(&out, &report)?;
        check_ivo_hull(&file, &source_uses(&rows, &report, "ship.cgf"))?;
        let authored = source_uses(&rows, &report, "Game2.dcb");
        assert!(
            authored
                .iter()
                .any(|row| row["location"]["recordIndex"] == 0
                    && row["location"]["field"].as_str().is_some_and(|field| {
                        field.ends_with(".Geometry.Geometry.Geometry.path")
                    })),
            "a suppressed ancestor's authored model selection contributes to its actual helper",
        );
        assert_eq!(
            report["artifactOrigin"]["containsEncryptedSource"],
            json!(encrypted.then_some(true)),
            "the only encrypted input is the ancestor of drawn pieces",
        );
        let image = fs::read(&out)?;
        match &baseline {
            None => baseline = Some(image),
            Some(previous) if !picture_changed => {
                assert_eq!(previous, &image, "losing nodes leave the PNG unchanged");
            }
            Some(previous) => assert_ne!(
                previous, &image,
                "the actual selected placement changes the PNG"
            ),
        }
    }
    Ok(())
}

fn check_ivo_hull(file: &ChunkFile<&[u8]>, uses: &[&Value]) -> TestResult {
    let (chunk, row) = file
        .chunks_of(ivo::NODE_MESH_COMBO)
        .next()
        .ok_or("node chunk")?;
    let records = row.offset() + 32;
    let expected_ordinals = [0, 2, 3];
    assert_eq!(
        uses.len(),
        8,
        "a suppressed model contributes only its selected names and routes"
    );
    for ordinal in expected_ordinals {
        let at = records + 208 * ordinal;
        assert_site(
            uses,
            chunk,
            ordinal,
            "parent",
            at + 148..at + 150,
            "selector",
        );
        assert_site(
            uses,
            chunk,
            ordinal,
            "localTransform",
            at + 80..at + 128,
            "conversion",
        );
    }
    let payload = file.chunk_data(chunk).ok_or("node payload")?;
    for (ordinal, name) in [
        (2, b"hp_nose\0".as_slice()),
        (3, b"hardpoint_relay\0".as_slice()),
    ] {
        let at = payload
            .windows(name.len())
            .position(|window| window == name)
            .ok_or("original name")?;
        let start = row.offset() + u64::try_from(at)?;
        let end = start + u64::try_from(name.len())?;
        assert_site(uses, chunk, ordinal, "name", start..end, "selector");
    }
    Ok(())
}

fn assert_site(
    uses: &[&Value],
    chunk: usize,
    ordinal: u64,
    field: &str,
    bytes: Range<u64>,
    role: &str,
) {
    let location = json!({
        "type": "subresource",
        "namespace": PLACEMENT,
        "indices": [chunk.to_string(), ordinal.to_string(), bytes.start.to_string(), bytes.end.to_string()],
        "field": field,
    });
    assert!(
        uses.iter()
            .any(|row| row["role"] == role && row["location"] == location),
        "missing {location} as {role}: {uses:?}",
    );
}

fn crch_parent() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = TestCrchModel::cube().bytes();
    let file = ChunkFile::parse(bytes.as_slice())?;
    let chunk = file.chunks().get(5).ok_or("CrCh child node")?;
    let at = usize::try_from(chunk.offset())?;
    bytes
        .get_mut(at..at + 9)
        .ok_or("name")?
        .copy_from_slice(b"hp_nose\0\0");
    bytes.get_mut(at + 64..at + 68).ok_or("object ID")?.fill(0);
    Ok(bytes)
}

fn offset_entries(parent: Vec<u8>, child: Vec<u8>) -> Vec<TestEntry> {
    let root = TestEntity::new(1, "TEST_Ship", "entities/spaceships/test_ship.xml")
        .geometry("Objects/Test/ship.cgf", None)
        .port(
            TestPort::new("socket")
                .helper("hp_nose", [0.0; 3], [0.0; 3])
                .item_helper("item_socket"),
        )
        .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
            "socket",
            "TEST_Child",
        )]));
    let child_entity = TestEntity::new(2, "TEST_Child", "entities/scitem/test_child.xml")
        .geometry("Objects/Test/child.cgf", None);
    let database = entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(child_entity.record())
        .build();
    vec![
        TestEntry::zstd("Data\\Game2.dcb", database),
        TestEntry::stored(SHIP, parent).encrypted(),
        TestEntry::stored(CHILD, child),
    ]
}

fn offset_model(invertible: bool) -> Vec<u8> {
    let mut socket = model_node("item_socket", Some(0), 3.0, 2);
    if !invertible {
        socket.local[0][0] = 0.0;
    }
    TestModel::cube()
        .single_file()
        .with_nodes(vec![
            model_node("item", None, 0.0, 2),
            model_node("item_mesh", Some(0), 0.0, 0),
            socket,
            model_node("ITEM_SOCKET", Some(0), 90.0, 2),
        ])
        .cgf()
}

#[test]
fn crch_parent_ids_and_only_successful_child_inversions_reach_the_receipt() -> TestResult {
    let dir = tempfile::tempdir()?;
    let key = test_key().to_hex();
    let out = dir.path().join("ship.png");
    let parent = crch_parent()?;
    let crch = ChunkFile::parse(parent.as_slice())?;
    for invertible in [true, false] {
        let input = tempfile::tempdir_in(dir.path())?;
        let bytes = offset_model(invertible);
        let child = ChunkFile::parse(bytes.as_slice())?;
        let archive = write_archive(input.path(), offset_entries(parent.clone(), bytes.clone()))?;
        let run = hologram(&archive, &out, &["--size", "96", "--key", &key], true)?;
        if skip(
            "crch_parent_ids_and_only_successful_child_inversions_reach_the_receipt",
            &run,
        ) {
            return Ok(());
        }
        assert_eq!(run.code, Some(0), "{}", run.stderr);
        let report: Value = serde_json::from_str(&run.stdout)?;
        let rows = receipt(&out, &report)?;
        check_crch_anchor(&crch, &source_uses(&rows, &report, "ship.cgf"))?;
        check_offset(
            &child,
            &source_uses(&rows, &report, "child.cgf"),
            invertible,
        )?;
        assert_eq!(
            report["drawn_parts"], 1,
            "the CrCh helper file supplies no drawn mesh"
        );
        assert_eq!(report["artifactOrigin"]["containsEncryptedSource"], true);
    }
    Ok(())
}

fn check_crch_anchor(file: &ChunkFile<&[u8]>, uses: &[&Value]) -> TestResult {
    assert_eq!(uses.len(), 6, "only the retained CrCh helper route");
    let root = file.chunks().get(2).ok_or("root chunk")?.offset();
    let helper = file.chunks().get(5).ok_or("helper chunk")?.offset();
    let root_id = file
        .table_row(2)
        .and_then(|row| row.id)
        .ok_or("root chunk ID")?;
    assert_site(uses, 5, 1, "name", helper..helper + 8, "selector");
    assert_site(uses, 5, 1, "parent", helper + 68..helper + 72, "selector");
    assert_site(
        uses,
        5,
        1,
        "localTransform",
        helper + 84..helper + 148,
        "conversion",
    );
    assert_site(uses, 2, 0, "chunkId", root_id, "selector");
    assert_site(uses, 2, 0, "parent", root + 68..root + 72, "selector");
    assert_site(
        uses,
        2,
        0,
        "localTransform",
        root + 84..root + 148,
        "conversion",
    );
    Ok(())
}

fn check_offset(file: &ChunkFile<&[u8]>, uses: &[&Value], invertible: bool) -> TestResult {
    let bindings: Vec<_> = uses
        .iter()
        .copied()
        .filter(|row| row.pointer("/location/namespace").and_then(Value::as_str) == Some(PLACEMENT))
        .collect();
    if !invertible {
        assert!(
            bindings.is_empty(),
            "singular offsets do not affect placement"
        );
        return Ok(());
    }
    assert_eq!(
        bindings.len(),
        5,
        "the first selected offset and its ancestor only"
    );
    let (chunk, node_table) = file
        .chunks_of(ivo::NODE_MESH_COMBO)
        .next()
        .ok_or("node table")?;
    let root = node_table.offset() + 32;
    let socket = root + 2 * 208;
    assert_site(
        &bindings,
        chunk,
        2,
        "localTransform",
        socket + 80..socket + 128,
        "conversion",
    );
    assert_site(
        &bindings,
        chunk,
        0,
        "localTransform",
        root + 80..root + 128,
        "conversion",
    );
    Ok(())
}
