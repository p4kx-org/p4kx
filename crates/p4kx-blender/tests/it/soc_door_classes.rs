// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::SocContainerCapture;
use p4kx_blender::{SocScene, soc_scene_schema};
use p4kx_extraction::{Archive, CancelToken};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestChunkFile, TestContainer, TestEntity, TestEntry,
    TestInstance, TestLoadout, TestStruct, TestType, TestValue, entity_schema,
};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const CLASS: u128 = 0x2222_2222_2222_2222_2222_2222_2222_2222;
const CLASS_GUID: &str = "22222222-2222-2222-2222-222222222222";
const SENSOR: u128 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
const SENSOR_GUID: &str = "33333333-3333-3333-3333-333333333333";
const MANNEQUIN: &str = "Data/Animations/Mannequin/ADB";
const CONTROLLER: &str = "synthetic/controller.xml";
const DATABASE: &str = "synthetic/door.adb";

fn forge() -> Vec<u8> {
    let controller = TestInstance::new("SAnimationControllerParams")
        .value("AnimationController", TestValue::text(CONTROLLER))
        .value("AnimationDatabase", TestValue::text(DATABASE));
    let door = TestInstance::new("SCItemDoorParams").value("FragName", TestValue::text("DoorOpen"));
    let class = TestEntity::new(
        CLASS,
        "SyntheticDoor",
        "libs/foundry/records/entities/test/door",
    )
    .component(TestValue::Struct(controller))
    .component(TestValue::Struct(door))
    .loadout(TestLoadout::Xml("test/panels.xml".into()));
    entity_schema(DataForgeBuilder::new())
        .structure(
            TestStruct::new("SAnimationControllerParams")
                .parent("DataForgeComponentParams")
                .field("AnimationController", TestType::String)
                .field("AnimationDatabase", TestType::String),
        )
        .structure(
            TestStruct::new("SCItemDoorParams")
                .parent("DataForgeComponentParams")
                .field("FragName", TestType::String),
        )
        .structure(
            TestStruct::new("SCItemProximitySensorBoxParams")
                .field("Extent", TestType::Class("Vec3".to_owned()))
                .field("Offset", TestType::Class("Vec3".to_owned())),
        )
        .structure(
            TestStruct::new("SCItemProximitySensorParams")
                .parent("DataForgeComponentParams")
                .field("exitDelay", TestType::Single)
                .field(
                    "SensorShape",
                    TestType::StrongPointer("SCItemProximitySensorBoxParams".to_owned()),
                ),
        )
        .record(class.record())
        .record(sensor().record())
        .build()
}

fn sensor() -> TestEntity {
    let vector = TestInstance::new("Vec3")
        .value("x", TestValue::Single(0.25))
        .value("y", TestValue::Single(0.5))
        .value("z", TestValue::Single(1.0));
    let shape = TestInstance::new("SCItemProximitySensorBoxParams")
        .value("Extent", TestValue::Struct(vector));
    let sensor = TestInstance::new("SCItemProximitySensorParams")
        .value("exitDelay", TestValue::Single(0.75))
        .value("SensorShape", TestValue::Struct(shape));
    TestEntity::new(
        SENSOR,
        "SyntheticSensor",
        "libs/foundry/records/entities/test/sensor",
    )
    .component(TestValue::Struct(sensor))
}

fn source() -> TestResult<Value> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("soc-class.p4k");
    let xml = format!(
        r#"<Entities><Entity EntityClassGUID="{CLASS_GUID}">
          <PropertiesDataCore>
            <SCItemDoor/>
            <EntityGeometryResource><Geometry><Geometry><Geometry path="test/door.cga"/></Geometry></Geometry></EntityGeometryResource>
          </PropertiesDataCore>
        </Entity><Entity EntityClassGUID="{SENSOR_GUID}" Pos="1,2,3"/></Entities>"#
    );
    let soc = TestChunkFile::crch()
        .chunk(0x0004, 3, xml.into_bytes())
        .bytes();
    let container = TestContainer::new()
        .entry(TestEntry::stored("door.xml", br#"<ObjectContainer><StreamingOCs><OC name="door.soc"/></StreamingOCs></ObjectContainer>"#))
        .entry(TestEntry::stored("door.soc", soc));
    let controller = br#"<ControllerDef><FragmentDefs><DoorOpen scopes="DoorScope"/></FragmentDefs></ControllerDef>"#;
    let database = br#"<AnimDB><FragmentList><DoorOpen><Fragment><AnimLayer><Animation name="Door_Authored"/></AnimLayer></Fragment></DoorOpen></FragmentList></AnimDB>"#;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("door.socpak", container.build()))
        .entry(TestEntry::stored("Data/Game2.dcb", forge()).encrypted())
        .entry(TestEntry::stored(format!("{MANNEQUIN}/{CONTROLLER}"), controller))
        .entry(TestEntry::stored(format!("{MANNEQUIN}/{DATABASE}"), database))
        .entry(TestEntry::stored("Data/test/panels.xml", br#"<Loadout><Items><Item portName="PanelPort" itemName="PanelClass"><Items/></Item></Items></Loadout>"#))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let container =
        archive.open_container(archive.entries().first().ok_or("original container")?)?;
    let cancel = CancelToken::new();
    let capture = SocContainerCapture::read(&[&archive, &container], &cancel)?;
    Ok(serde_json::to_value(SocScene::from_archive_capture(
        &capture, &archive, &cancel,
    )?)?)
}

#[test]
fn soc_door_class_joins_exact_fragment_and_panel_port_sources() -> TestResult {
    let scene = source()?;
    let schema = soc_scene_schema();
    jsonschema::validator_for(&schema)?
        .validate(&scene)
        .map_err(|error| error.to_string())?;
    verify_instance(&scene)?;
    verify_class(&scene)?;
    verify_sensor(&scene)
}

fn verify_instance(scene: &Value) -> TestResult {
    let entity = scene
        .pointer("/occurrences/0/streams/0/member/data/0/entities/0")
        .ok_or("source door")?;
    assert_eq!(
        entity.pointer("/class/value/type").and_then(Value::as_str),
        Some("matched"),
        "class GUID has one physical target"
    );
    assert_eq!(
        entity
            .pointer("/class/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "encrypted class source affects the join"
    );
    assert_eq!(
        entity
            .pointer("/components/1/geometryPaths/0/value")
            .and_then(Value::as_str),
        Some("test/door.cga"),
        "exact inline door geometry"
    );
    Ok(())
}

fn verify_class(scene: &Value) -> TestResult {
    let components = scene
        .pointer("/classes/0/components/value")
        .and_then(Value::as_array)
        .ok_or("source class components")?;
    let find = |kind: &str| {
        components
            .iter()
            .find(|component| component.get("type").and_then(Value::as_str) == Some(kind))
    };
    let animation = find("animationController").ok_or("class animation controller")?;
    let clip = animation
        .pointer("/database/value/data/declaredFragments/0/animations/0")
        .ok_or("authored animation selector")?;
    assert_eq!(
        clip.pointer("/name/value").and_then(Value::as_str),
        Some("Door_Authored"),
        "exact XML animation selector"
    );
    assert_eq!(
        clip.pointer("/physicalClip/value").and_then(Value::as_str),
        Some("unresolved"),
        "no runtime clip inferred from a name"
    );
    let loadout = find("defaultLoadout").ok_or("class default loadout")?;
    assert_eq!(
        loadout
            .pointer("/loadout/xml/value/data/items/0/port/value")
            .and_then(Value::as_str),
        Some("PanelPort"),
        "authored class panel port"
    );
    assert_eq!(
        loadout
            .pointer("/loadout/xml/value/data/items/0/item/value")
            .and_then(Value::as_str),
        Some("PanelClass"),
        "authored class panel target"
    );
    assert_eq!(
        loadout
            .pointer("/loadout/xml/value/data/items/0/itemGroups")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "authored empty nested Items group has a physical position"
    );
    Ok(())
}

fn verify_sensor(scene: &Value) -> TestResult {
    let placement = scene
        .pointer("/occurrences/0/streams/0/member/data/0/entities/1")
        .ok_or("class-only sensor placement")?;
    assert_eq!(
        placement
            .pointer("/classSensor/value")
            .and_then(Value::as_bool),
        Some(true),
        "the exact class component establishes the sensor without an inline declaration"
    );
    assert_eq!(
        placement
            .pointer("/classSensor/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "class-only sensor identity includes its encrypted class contribution"
    );
    let sensor = scene
        .pointer("/classes/1/components/value/0/sensor")
        .ok_or("sensor class")?;
    assert_eq!(
        sensor.pointer("/exitDelay/value").and_then(Value::as_f64),
        Some(0.75),
        "authored exit delay"
    );
    let extent = sensor
        .pointer("/boxShape/extent/value/x")
        .ok_or("sensor extent axis")?;
    assert_eq!(
        extent.get("value").and_then(Value::as_f64),
        Some(0.25),
        "original sensor-local extent"
    );
    assert_eq!(
        extent
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "sensor extent has its own original encryption marker"
    );
    Ok(())
}
