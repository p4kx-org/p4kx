// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::BodyVariant;
use p4kx_assembly::character::lowered_hash;
use p4kx_blender::{PlayerSource, player_source_schema};
use p4kx_extraction::{Archive, CancelToken};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestChf, TestEntity, TestEntry, TestInstance, TestItemPort,
    TestPort, TestStruct, TestType, TestValue, entity_schema,
};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[path = "player_motion_option_assertions.rs"]
mod motion_option_assertions;

#[path = "player_subadb_graph_assertions.rs"]
mod subadb_graph_assertions;

#[path = "player_shared_capture.rs"]
mod shared_capture;

const BODY: u128 = 0x0101_0101_0101_0101_0101_0101_0101_0101;
const WEAPON_ONE: u128 = 0x0202_0202_0202_0202_0202_0202_0202_0202;
const WEAPON_TWO: u128 = 0x0303_0303_0303_0303_0303_0303_0303_0303;

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("player.p4k");
    let mut male = TestChf::v7();
    male.ports = Some(port(
        1,
        1,
        vec![
            port(lowered_hash("nested"), 0, vec![]),
            port(
                lowered_hash("stowed"),
                2,
                vec![port(lowered_hash("nested"), 0, vec![])],
            ),
        ],
    ));
    let mut female = TestChf::v8();
    female.ports = Some(port(1, 1, vec![port(lowered_hash("missing"), 4, vec![])]));
    let loadout = br#"<Loadout><Items><Item portName="root" itemName=""><Items>
        <Item portName="stowed" itemName="SharedWeapon"/>
        <Item portName="missing" itemName="MissingClass"/>
        <Item portName="unspecified"><Items/></Item>
    </Items></Item><Item portName="second" itemName="SyntheticBody"><Items>
        <Item portName="nested" itemName="SharedWeapon"/>
        <Item portName="unknown" itemName=""/>
    </Items></Item></Items><Inventory><InventoryItem itemName="Other"/></Inventory></Loadout>"#;
    let archive = ArchiveBuilder::new()
        .entry(
            TestEntry::stored(
                "Data/Libs/CharacterCustomizer/masculinedefault.chf",
                male.to_bytes(),
            )
            .encrypted(),
        )
        .entry(TestEntry::stored(
            "Data/Libs/CharacterCustomizer/femininedefault.chf",
            female.to_bytes(),
        ))
        .entry(TestEntry::stored("Data/Game2.dcb", forge()).encrypted())
        .entry(TestEntry::stored(
            "Data/Scripts/Loadouts/Player/DefaultLoadouts/sample.xml",
            loadout,
        ));
    motion_option_assertions::entries()
        .into_iter()
        .chain(subadb_graph_assertions::entries())
        .fold(archive, ArchiveBuilder::entry)
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn forge() -> Vec<u8> {
    let animated = TestInstance::new("SAnimatedCharacterComponentParams")
        .value(
            "actionController",
            TestValue::text("Animations/test/controller.xml"),
        )
        .value(
            "animDatabase3P",
            TestValue::text("Animations/test/player.adb"),
        );
    let actor = TestEntity::new(4, "Player", "libs/foundry/records/entities/test/player")
        .component(TestValue::Struct(animated));
    entity_schema(DataForgeBuilder::new())
        .structure(
            TestStruct::new("SAnimatedCharacterComponentParams")
                .parent("DataForgeComponentParams")
                .field("actionController", TestType::String)
                .field("animDatabase3P", TestType::String),
        )
        .record(actor.record())
        .record(
            TestEntity::new(
                BODY,
                "SyntheticBody",
                "libs/foundry/records/entities/test/body",
            )
            .port(
                TestPort::new("nested")
                    .skin()
                    .helper("BodyAnchor", [1.0, 2.0, 3.0], [0.0; 3]),
            )
            .port(
                TestPort::new("stowed")
                    .flags("invisible")
                    .helper("StowOne", [0.0; 3], [0.0; 3]),
            )
            .port(TestPort::new("STOWED").helper("StowTwo", [0.0; 3], [0.0; 3]))
            .record(),
        )
        .record(
            TestEntity::new(
                WEAPON_ONE,
                "SharedWeapon",
                "libs/foundry/records/entities/test/weapon-a",
            )
            .port(TestPort::new("nested").helper("WeaponAnchor", [0.0; 3], [0.0; 3]))
            .record(),
        )
        .record(
            TestEntity::new(
                WEAPON_TWO,
                "SharedWeapon",
                "libs/foundry/records/entities/test/weapon-b",
            )
            .record(),
        )
        .build()
}

const fn port(port: u32, item: u8, children: Vec<TestItemPort>) -> TestItemPort {
    TestItemPort {
        port,
        item: [item; 16],
        tail_count: 0,
        children,
    }
}

#[test]
fn player_source_preserves_body_variant_port_order_and_exact_loadout_targets() -> TestResult {
    let (_folder, archive) = fixture()?;
    let cancel = CancelToken::new();
    let male = serde_json::to_value(PlayerSource::read(&archive, BodyVariant::Male, &cancel)?)?;
    let female = serde_json::to_value(PlayerSource::read(&archive, BodyVariant::Female, &cancel)?)?;
    let validator = jsonschema::validator_for(&player_source_schema())?;
    validator
        .validate(&male)
        .map_err(|error| error.to_string())?;
    validator
        .validate(&female)
        .map_err(|error| error.to_string())?;
    verify_bodies(&male, &female);
    verify_port_definitions(&male, &female);
    verify_loadout(&male);
    verify_xml_port_definitions(&male);
    verify_motion(&male);
    motion_option_assertions::verify(&male)?;
    subadb_graph_assertions::verify(&male)?;
    Ok(())
}

fn verify_bodies(male: &Value, female: &Value) {
    assert_eq!(
        male.pointer("/bodyVariant").and_then(Value::as_str),
        Some("male"),
        "explicit male source"
    );
    assert_eq!(
        female.pointer("/bodyVariant").and_then(Value::as_str),
        Some("female"),
        "explicit female source"
    );
    assert_eq!(
        male.pointer("/body/value/data/ports/value/1/parent")
            .and_then(Value::as_u64),
        Some(0),
        "physical parent slot"
    );
    assert_eq!(
        male.pointer("/body/value/data/ports/value/1/item/value/type")
            .and_then(Value::as_str),
        Some("empty"),
        "authored empty CHF port"
    );
    assert_eq!(
        male.pointer("/body/value/data/ports/value/2/item/value/type")
            .and_then(Value::as_str),
        Some("matched"),
        "exact CHF GUID"
    );
    assert_eq!(
        female
            .pointer("/body/value/data/ports/value/1/item/value/type")
            .and_then(Value::as_str),
        Some("unmatched"),
        "unknown GUID is retained"
    );
    assert_eq!(
        male.pointer("/body/value/data/ports/value/2/item/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "original CHF and DataForge evidence"
    );
}

fn verify_port_definitions(male: &Value, female: &Value) {
    let paths = "/body/value/data/ports/value";
    let value = |doc: &Value, pointer: &str| {
        doc.pointer(&format!("{paths}/{pointer}"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    assert_eq!(
        value(male, "0/definition/value/type").as_deref(),
        Some("noOwner"),
        "CHF root has no parent"
    );
    assert_eq!(
        value(
            male,
            "1/definition/value/definition/attachment/value/helpers/value/0/name/value"
        )
        .as_deref(),
        Some("BodyAnchor"),
        "same CRC resolves on body"
    );
    assert_eq!(
        value(male, "2/definition/value/type").as_deref(),
        Some("ambiguousPort"),
        "both original same-name definitions remain visible"
    );
    assert_eq!(
        value(
            male,
            "3/definition/value/definition/attachment/value/helpers/value/0/name/value"
        )
        .as_deref(),
        Some("WeaponAnchor"),
        "same CRC resolves on immediate weapon owner"
    );
    assert_eq!(
        value(female, "1/definition/value/type").as_deref(),
        Some("unmatchedPort"),
        "missing owner-local declaration remains explicit"
    );
}

fn verify_xml_port_definitions(doc: &Value) {
    let value = |index, field| {
        doc.pointer(&format!(
            "/loadouts/0/value/declarations/{index}/definition/value/{field}"
        ))
        .and_then(Value::as_str)
    };
    assert_eq!(
        value(1, "type"),
        Some("noOwner"),
        "root-group declaration has no authored owner"
    );
    assert_eq!(
        value(3, "type"),
        Some("emptyOwner"),
        "explicit empty parent remains separate"
    );
    assert_eq!(
        value(9, "type"),
        Some("matched"),
        "child port on the named physical owner"
    );
    assert_eq!(
        value(9, "definition/attachment/value/helpers/value/0/name/value"),
        Some("BodyAnchor"),
        "the XML class owner supplies the same original physical port"
    );
    assert_eq!(
        value(10, "type"),
        Some("unmatchedPort"),
        "a name absent from that owner stays unresolved"
    );
}

fn verify_loadout(doc: &Value) {
    let rows = "/loadouts/0/value/declarations";
    assert_eq!(
        doc.pointer(&format!("{rows}/1/target/value/type"))
            .and_then(Value::as_str),
        Some("empty"),
        "empty original itemName"
    );
    assert_eq!(
        doc.pointer(&format!("{rows}/3/target/value/records"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "both same-name weapon records"
    );
    assert_eq!(
        doc.pointer(&format!("{rows}/4/target/value/type"))
            .and_then(Value::as_str),
        Some("unmatched"),
        "named missing original target"
    );
    assert_eq!(
        doc.pointer(&format!("{rows}/5/target/state"))
            .and_then(Value::as_str),
        Some("absent"),
        "omitted itemName differs from empty"
    );
    assert_eq!(
        doc.pointer(&format!("{rows}/6/parent"))
            .and_then(Value::as_u64),
        Some(5),
        "explicit empty nested Items group"
    );
    assert!(
        doc.pointer("/documents/0/nodes")
            .and_then(Value::as_array)
            .is_some_and(|nodes| nodes
                .iter()
                .any(|node| node.get("name").and_then(Value::as_str) == Some("InventoryItem"))),
        "additional original sections retained"
    );
}

fn verify_motion(doc: &Value) {
    let component = "/motion/components/value/0";
    assert_eq!(
        doc.pointer(&format!("{component}/controller/value/type"))
            .and_then(Value::as_str),
        Some("present"),
        "player actor selects the physically admitted controller"
    );
    assert_eq!(
        doc.pointer(&format!("{component}/thirdPersonDatabase/value/type"))
            .and_then(Value::as_str),
        Some("present"),
        "player actor selects the physically admitted ADB"
    );
    assert_eq!(
        doc.pointer(&format!("{component}/controller/containsEncryptedSource"))
            .and_then(Value::as_bool),
        Some(true),
        "the encrypted original actor selector contributes to the XML join"
    );
    assert_eq!(
        doc.pointer("/documents/1/nodes")
            .and_then(Value::as_array)
            .map(|nodes| nodes
                .iter()
                .filter(|node| node.get("name").and_then(Value::as_str) == Some("Walk"))
                .count()),
        Some(2),
        "repeated controller declarations remain ordered in the original document"
    );
}

#[test]
fn checked_in_player_source_schema_is_current_and_valid() -> TestResult {
    let schema = player_source_schema();
    jsonschema::draft202012::meta::validate(&schema).map_err(|error| error.to_string())?;
    let mut generated = serde_json::to_string_pretty(&schema)?;
    generated.push('\n');
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/player-source.schema.json");
    if std::env::var_os("P4KX_BLESS").is_some_and(|value| value == "1") {
        std::fs::write(&path, &generated)?;
    }
    assert_eq!(
        std::fs::read_to_string(path)?,
        generated,
        "regenerate with P4KX_BLESS=1"
    );
    Ok(())
}
