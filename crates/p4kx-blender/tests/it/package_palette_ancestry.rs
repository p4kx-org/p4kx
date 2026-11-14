// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestModel,
    entity_schema, tint_palette_record,
};
use serde_json::Value;

use crate::support::{TestResult, fixture};

use super::{binding, field, package};

#[test]
fn palette_origin_crosses_model_read_error_and_geometryless_intermediates() -> TestResult {
    let fixture = fixture(inheritance_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let scratch = tempfile::tempdir()?;
    let manifest = package(&archive, &archive, "TEST_Ancestor", scratch.path())?;
    let root = binding(&manifest, "")?;
    let leaf = binding(&manifest, "unreadable/geometryless/drawn")?;
    assert_eq!(
        leaf.get("inheritedFrom"),
        root.get("node"),
        "the first actual palette supplier"
    );
    assert_eq!(
        leaf.get("payload"),
        root.get("payload"),
        "both draw the same palette values"
    );
    assert_eq!(
        leaf.get("selection"),
        root.get("selection"),
        "both refer to the same authored choice"
    );
    let nodes = field(&manifest, "/nodes")?
        .as_array()
        .ok_or("package nodes")?;
    for path in ["unreadable", "unreadable/geometryless"] {
        let node = nodes
            .iter()
            .find(|node| node.get("path").and_then(Value::as_str) == Some(path))
            .ok_or("retained intermediate node")?;
        assert!(
            node.get("mesh").is_none(),
            "{path} is retained as a meshless node"
        );
        assert!(
            binding(&manifest, path).is_err(),
            "{path} never becomes a palette supplier"
        );
    }
    let issues = field(&manifest, "/issues")?
        .as_array()
        .ok_or("package issues")?;
    assert!(
        issues
            .iter()
            .any(|issue| issue.get("kind").and_then(Value::as_str) == Some("geometry_unreadable")),
        "the corrupt model takes the model-read failure path"
    );
    Ok(())
}

fn inheritance_entries() -> Vec<TestEntry> {
    const MODEL: &str = "Objects/Test/palette-ancestry.cgf";
    const BROKEN: &str = "Objects/Test/broken-palette-ancestry.cgf";
    const PALETTE: u128 = 5;
    let attach = |port, class| TestLoadout::Manual(vec![TestLoadoutEntry::class(port, class)]);
    let entities = [
        TestEntity::new(1, "TEST_Ancestor", "entities/ancestor.xml")
            .geometry(MODEL, None)
            .palette(PALETTE, "")
            .loadout(attach("unreadable", "TEST_Unreadable")),
        TestEntity::new(2, "TEST_Unreadable", "entities/unreadable.xml")
            .geometry(BROKEN, None)
            .palette(PALETTE, "")
            .loadout(attach("geometryless", "TEST_Geometryless")),
        TestEntity::new(3, "TEST_Geometryless", "entities/geometryless.xml")
            .loadout(attach("drawn", "TEST_Drawn")),
        TestEntity::new(4, "TEST_Drawn", "entities/drawn.xml").geometry(MODEL, None),
    ];
    let builder = entities
        .iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        });
    let database = builder.record(tint_palette_record(
        PALETTE,
        "ancestry",
        [[255, 0, 0]; 3],
        [128; 3],
        [0, 32, 64],
    ));
    let mesh = TestModel::cube();
    vec![
        TestEntry::zstd("Data/Game2.dcb", database.build()),
        TestEntry::stored(format!("Data/{MODEL}"), mesh.cgf()),
        TestEntry::stored(format!("Data/{MODEL}m"), mesh.cgfm()),
        TestEntry::stored(format!("Data/{BROKEN}"), vec![0u8; 32]),
    ]
}
