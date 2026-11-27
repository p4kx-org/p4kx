// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, LoadoutResolver};
use p4kx_blender::{Manifest, PackageInfo, write_package};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestModel, TestPort, TestValue,
    entity_schema, test_key,
};
use serde_json::{Value, json};

use crate::support::{Fixture, TestResult, fixture, glb_options, never};

const ROOT: &str = "TEST_Classification";
const MODEL: &str = "Objects/Test/classification.cgf";
const INVALID_STRING_OFFSET: u32 = u32::MAX - 1;

#[test]
fn node_types_preserve_their_original_owners_and_dispositions() -> TestResult {
    let (_fixture, archive) = archive()?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&entry);
    let different_open = DataForgeEntry::read(&archive)?;
    assert!(
        different_open.origin_of(&forge).is_none(),
        "equal decoded bytes from a separate read cannot replace the retained origin"
    );
    let root = resolver.entities().find(ROOT)?;
    let tree = resolver.resolve(&root)?;
    let models = ArchiveModels::new(&archive);
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;
    let scratch = tempfile::tempdir()?;
    let retained = std::env::var_os("P4KX_SYNTHETIC_ENTITY_PACKAGE").map(PathBuf::from);
    let out = retained
        .as_deref()
        .unwrap_or_else(|| scratch.path())
        .join("classification");
    write_package(
        &assembly,
        &models,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::default(),
        &out,
        &never,
    )?;
    let raw: Value = serde_json::from_slice(&fs::read(out.join("scene.json"))?)?;
    verify(&raw, &resolver)?;
    let manifest: Manifest = serde_json::from_value(raw.clone())?;
    let parsed = serde_json::to_value(manifest)?;
    assert_eq!(parsed, raw, "typed Rust manifest round-trips");
    let old = without_classification(raw)?;
    assert!(
        old.nodes.iter().all(|node| node
            .entity
            .as_ref()
            .is_none_or(|entity| entity.classification.is_none())),
        "old manifests have no classification"
    );
    Ok(())
}

fn verify(raw: &Value, resolver: &LoadoutResolver<'_>) -> TestResult {
    let nodes = raw
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or("manifest nodes")?;
    let uses = raw
        .pointer("/entity_provenance/uses")
        .and_then(Value::as_array)
        .ok_or("classification uses")?;
    for (class, expected) in [
        (
            ROOT,
            [
                ("present", Some("RootCase")),
                ("present", Some("Authored Root")),
            ],
        ),
        (
            "TEST_Child",
            [("present", Some("ChildCase")), ("present", Some(""))],
        ),
        ("TEST_Absent", [("absent", None), ("absent", None)]),
        (
            "TEST_Unreadable",
            [("unreadable", None), ("present", Some("ReadableSibling"))],
        ),
    ] {
        let node = nodes
            .iter()
            .find(|node| node.pointer("/entity/class_name").and_then(Value::as_str) == Some(class))
            .ok_or("classified node")?;
        let record = resolver.entities().find(class)?.index();
        let selected = node
            .pointer("/entity/classification")
            .ok_or("classification")?;
        for ((key, field), (state, value)) in [("item_type", "Type"), ("subtype", "SubType")]
            .into_iter()
            .zip(expected)
        {
            verify_value(
                selected.get(key).ok_or("classification value")?,
                uses,
                (record, field),
                state,
                value,
            )?;
        }
        if class == "TEST_Absent" || class == "TEST_Unreadable" {
            assert!(
                node.get("mesh").is_none(),
                "classification survives no geometry"
            );
        }
    }
    verify_shared_mesh(nodes)
}

fn verify_shared_mesh(nodes: &[Value]) -> TestResult {
    let root = nodes
        .iter()
        .find(|node| node.get("path").and_then(Value::as_str) == Some(""))
        .ok_or("root node")?;
    let child = nodes
        .iter()
        .find(|node| node.get("path").and_then(Value::as_str) == Some("child"))
        .ok_or("child node")?;
    assert_eq!(
        root.get("mesh"),
        child.get("mesh"),
        "authored categories do not split shared meshes"
    );
    Ok(())
}

fn verify_value(
    value: &Value,
    uses: &[Value],
    (record, field): (u32, &str),
    state: &str,
    expected: Option<&str>,
) -> TestResult {
    assert_eq!(
        value.get("state"),
        Some(&json!(state)),
        "typed classification disposition"
    );
    assert_eq!(
        value.get("value"),
        Some(&json!(expected)),
        "authored spelling and empty value"
    );
    assert_eq!(
        value.get("containsEncryptedSource"),
        Some(&json!(true)),
        "original DataForge is encrypted"
    );
    assert_eq!(
        value.get("sourceSetComplete"),
        Some(&json!(state != "unreadable")),
        "an unreadable value retains its consumed offset but has incomplete proof"
    );
    let refs = value
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("selected source refs")?;
    let [reference] = refs.as_slice() else {
        return Err("one selected DataForge use".into());
    };
    let source = uses
        .iter()
        .find(|source| source.get("id") == Some(reference))
        .ok_or("bound use")?;
    let location = source.get("location").ok_or("source location")?;
    if state == "unreadable" {
        verify_invalid_offset(source, location, record, field);
        return Ok(());
    }
    assert_eq!(
        location.get("recordIndex"),
        Some(&json!(record)),
        "selected owning record"
    );
    let path = location
        .get("field")
        .and_then(Value::as_str)
        .ok_or("source coordinate")?;
    if state == "absent" {
        assert_eq!(
            path, "Components",
            "observed no AttachDef owner in selected record"
        );
    } else {
        assert!(
            path.starts_with("Components[") && path.ends_with(&format!("].AttachDef.{field}")),
            "exact classification field {path}"
        );
    }
    Ok(())
}

fn verify_invalid_offset(source: &Value, location: &Value, record: u32, field: &str) {
    assert_eq!(
        source.get("role"),
        Some(&json!("conversion")),
        "the invalid original offset produces the unreadable state"
    );
    assert_eq!(
        location.get("namespace"),
        Some(&json!("dataForgeStringOffset")),
        "only the original string-offset slot contributes"
    );
    assert_eq!(
        location.pointer("/indices/0"),
        Some(&json!(record.to_string())),
        "the consumed slot belongs to the exact selected record"
    );
    assert_eq!(
        location.pointer("/indices/2"),
        Some(&json!(INVALID_STRING_OFFSET.to_string())),
        "the retained little-endian offset is the one consumed by the decoder"
    );
    assert_eq!(
        location.get("field"),
        Some(&json!(format!("AttachDef.{field}"))),
        "the unread text target is not registered"
    );
}

fn without_classification(mut wire: Value) -> Result<Manifest, Box<dyn Error>> {
    let manifest = wire.as_object_mut().ok_or("package object")?;
    manifest.remove("entity_provenance");
    manifest.insert("version".into(), json!("1.15"));
    for node in manifest
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or("old package nodes")?
    {
        if let Some(entity) = node.get_mut("entity").and_then(Value::as_object_mut) {
            entity.remove("classification");
        }
    }
    Ok(serde_json::from_value(wire)?)
}

fn archive() -> Result<(Fixture, Archive), Box<dyn Error>> {
    let child = TestEntity::new(2, "TEST_Child", "qa/child.xml")
        .attach_type("ChildCase")
        .sub_type("")
        .geometry(MODEL, None);
    let missing = TestEntity::new(3, "TEST_Absent", "qa/missing.xml");
    let unreadable =
        TestEntity::new(4, "TEST_Unreadable", "qa/unreadable.xml").component(TestValue::Struct(
            TestInstance::new("SAttachableComponentParams").value(
                "AttachDef",
                TestValue::Struct(
                    TestInstance::new("SItemDefinition")
                        .value(
                            "Type",
                            TestValue::Raw(INVALID_STRING_OFFSET.to_le_bytes().to_vec()),
                        )
                        .value("SubType", TestValue::text("ReadableSibling")),
                ),
            ),
        ));
    let root = TestEntity::new(1, ROOT, "qa/root.xml")
        .attach_type("RootCase")
        .sub_type("Authored Root")
        .geometry(MODEL, None)
        .port(TestPort::new("child").default_item(2))
        .port(TestPort::new("absent").default_item(3))
        .port(TestPort::new("unreadable").default_item(4));
    let forge = entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(child.record())
        .record(missing.record())
        .record(unreadable.record())
        .build();
    let model = TestModel::cube();
    let fixture = fixture(vec![
        TestEntry::stored("Data/Game2.dcb", forge).encrypted(),
        TestEntry::stored(format!("Data/{MODEL}"), model.cgf()),
        TestEntry::stored(
            format!("Data/{}", Path::new(MODEL).with_extension("cgfm").display()),
            model.cgfm(),
        ),
    ])?;
    let archive = Archive::open_with_key(&fixture.path, Some(test_key()))?;
    Ok((fixture, archive))
}
