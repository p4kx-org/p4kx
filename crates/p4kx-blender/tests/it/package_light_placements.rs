// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::PathBuf;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, EntityInterior, InteriorOptions,
    LoadoutResolver,
};
use p4kx_blender::{Manifest, PackageInfo, manifest_schema, write_package};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    DataForgeBuilder, TestChunkFile, TestContainer, TestEntity, TestEntry, TestModel, TestNode,
    entity_schema, test_key,
};
use serde_json::{Value, json};

use crate::support::{TestResult, fixture, glb_options, never};

const CLASS: &str = "TEST_TileLightPlacement";
const VEHICLE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Light.xml";
const CONTAINER: &str = "ObjectContainers/Ships/Test/lights.socpak";
const MODEL: &str = "Objects/Test/tile_owner.cgf";
const CUBE: [[f32; 4]; 3] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];
const HELPER: [[f32; 4]; 3] = [
    [0.0, -3.0, 0.0, 10.0],
    [2.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];
const VEHICLE_XML: &str = r#"<Vehicle><Parts><Part name="Light" class="ItemPort"><ItemPort>
    <Types><Type type="MultiLight"/></Types><AttachmentPoints>
    <AttachmentPoint id="tip" name="tip" bone="absent"/>
    </AttachmentPoints></ItemPort></Part></Parts><Modifications><Modification name="Selected">
    <Elems><Elem idRef="tip" name="bone" value="light"/></Elems>
    </Modification></Modifications></Vehicle>"#;
const DESCRIPTOR: &str = r#"<ObjectContainer><StreamingOCs><OC name="lights.soc"/></StreamingOCs>
    <TileItemPortEntries><ItemPort name="fixture[tip]" interactionOffset="4,8,0"/></TileItemPortEntries>
    </ObjectContainer>"#;
const LIGHTS: &str = r#"<Entities><Entity Name="group" EntityClass="LightGroup" Pos="1,2,3">
   <TileAttachmentParams ItemPortName="fixture[tip]"/><EntityComponentLightGroup><BakedInLights>
   <Light Name="first"><RelativeXForm rotation="1,0,0,0" translation="1,0,0"/>
    <EntityComponentLight lightType="Omni"><defaultState intensity="0"/><emergencyState intensity="2"/></EntityComponentLight>
   </Light>
   <Light Name="second"><RelativeXForm rotation="1,0,0,0" translation="0,1,0"/>
    <EntityComponentLight lightType="Omni"><defaultState intensity="1"/></EntityComponentLight>
   </Light>
   </BakedInLights></EntityComponentLightGroup></Entity></Entities>"#;

#[test]
fn package_lights_join_exact_occurrences_and_field_origins() -> TestResult {
    let fixture = fixture(entries())?;
    let archive = Archive::open_with_key(fixture.path, Some(test_key()))?;
    let forge_entry = DataForgeEntry::read(&archive)?;
    let forge = forge_entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_forge_entry(&forge_entry)
        .with_files(&files);
    let tree = resolver.resolve(&resolver.entities().find(CLASS)?)?;
    let models = ArchiveModels::new(&archive);
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;
    let interior = EntityInterior::build(&archive, &assembly, &tree, &InteriorOptions::default())?;
    let assembly = assembly.with_interior(interior);
    let dir = tempfile::tempdir()?;
    let retained = std::env::var_os("P4KX_SYNTHETIC_TILE_LIGHT_PACKAGE").map(PathBuf::from);
    let out = retained
        .as_deref()
        .unwrap_or_else(|| dir.path())
        .join("tile_lights");
    let package = write_package(
        &assembly,
        &models,
        &models.textures(),
        &glb_options(0),
        &PackageInfo::default(),
        &out,
        &never,
    )?;
    assert_eq!(package.lights, 4, "{:?}", package.issues);
    let raw: Value = serde_json::from_slice(&fs::read(out.join("scene.json"))?)?;
    jsonschema::validator_for(&manifest_schema())?
        .validate(&raw)
        .map_err(|error| error.to_string())?;
    let parsed: Manifest = serde_json::from_value(raw.clone())?;
    assert_eq!(
        parsed.light_placements.as_ref(),
        raw.get("light_placements"),
        "the Rust reader retains placement evidence"
    );
    verify(&raw)
}

fn entries() -> Vec<TestEntry> {
    let root = TestEntity::new(1, CLASS, "entities/spaceships/synthetic_tile.xml")
        .vehicle(VEHICLE, "Selected")
        .geometry(MODEL, None)
        .object_container(CONTAINER, "container", [0.0; 3], [0.0; 3])
        .object_container(CONTAINER, "missing", [0.0; 3], [0.0; 3]);
    let forge = entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .build();
    let node = |name: &str, parent, local, kind| TestNode {
        name: name.to_owned(),
        parent,
        local,
        kind,
    };
    let model = TestModel::cube().with_nodes(vec![
        node("root", None, CUBE, 2),
        node("mesh", Some(0), CUBE, 0),
        node("container", Some(0), HELPER, 2),
        node("light", Some(0), CUBE, 2),
    ]);
    let raw = TestChunkFile::crch().chunk(4, 0, LIGHTS).bytes();
    let container = TestContainer::new()
        .entry(TestEntry::stored("lights.xml", DESCRIPTOR))
        .entry(TestEntry::stored("lights.soc", raw))
        .build();
    vec![
        TestEntry::stored("Data/Game2.dcb", forge),
        TestEntry::stored(format!("Data/{MODEL}"), model.cgf()),
        TestEntry::stored("Data/Objects/Test/tile_owner.cgfm", model.cgfm()),
        TestEntry::stored(format!("Data/{VEHICLE}"), VEHICLE_XML).encrypted(),
        TestEntry::stored(format!("Data/{CONTAINER}"), container).encrypted(),
    ]
}

fn verify(package: &Value) -> TestResult {
    let rows = package
        .pointer("/light_placements/rows")
        .and_then(Value::as_array)
        .ok_or("light placement rows")?;
    let [first, second, failed, _] = rows.as_slice() else {
        return Err("each original light occurs once in each owner occurrence".into());
    };
    assert_ne!(
        first.get("container"),
        failed.get("container"),
        "reused containers are distinct occurrences"
    );
    assert_eq!(
        first.get("container"),
        second.get("container"),
        "both baked lights share the declared container"
    );
    assert_eq!(
        first.get("owner"),
        failed.get("owner"),
        "both container placements belong to one entity"
    );
    let node = first
        .get("node")
        .and_then(Value::as_u64)
        .ok_or("actual node index")?;
    assert_eq!(
        package.pointer(&format!("/nodes/{node}/name")),
        Some(&json!("first")),
        "placement identifies the actual projected light"
    );
    for (placement, path, value) in [
        (first, "/preview_state/value", json!("emergency")),
        (first, "/original_light/value/0/3", json!(2.0)),
        (first, "/effective_light/value/0/3", json!(5.0)),
        (first, "/helper/source/origin", json!("elem")),
        (failed, "/anchor/reason", json!("missing_container_frame")),
        (failed, "/effective_light/value/0/3", json!(2.0)),
    ] {
        assert_eq!(
            placement.pointer(path),
            Some(&value),
            "exported placement field {path}"
        );
    }
    verify_origins(package, first)
}

fn verify_origins(package: &Value, first: &Value) -> TestResult {
    for (path, expected) in [
        ("/original_light/containsEncryptedSource", json!(true)),
        ("/effective_light/containsEncryptedSource", json!(true)),
        (
            "/anchor/port/interaction_offset/containsEncryptedSource",
            json!(true),
        ),
        ("/helper/name/containsEncryptedSource", json!(true)),
        (
            "/anchor/owner_from_container/containsEncryptedSource",
            Value::Null,
        ),
    ] {
        assert_eq!(
            first.pointer(path),
            Some(&expected),
            "immediate origin marker for {path}"
        );
    }
    let uses = package
        .pointer("/light_placements/sourceInventory/uses")
        .and_then(Value::as_array)
        .ok_or("source use registry")?;
    let ids = first
        .pointer("/helper/name/sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("helper name origins")?;
    assert!(
        ids.iter().any(|id| uses.iter().any(|used| {
            used.get("id") == Some(id) && used.pointer("/location/field") == Some(&json!("value"))
        })),
        "the effective helper points to the winning original Elem value attribute"
    );
    Ok(())
}
