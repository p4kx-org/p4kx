// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::SocContainerCapture;
use p4kx_blender::{SocScene, soc_scene_schema};
use p4kx_extraction::{Archive, CancelToken};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestChunkFile, TestContainer, TestEntity, TestEntry,
    TestInstance, TestStruct, TestType, TestValue, entity_schema,
};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const CLASS: u128 = 0x2222_2222_2222_2222_2222_2222_2222_2222;

fn forge() -> Vec<u8> {
    let mode = TestInstance::new("FixtureLightMode")
        .value("intensity", TestValue::Single(3.0))
        .value("temperature", TestValue::Single(4800.0))
        .value("lightStyle", TestValue::UInt8(2));
    let component = TestInstance::new("LightComponentParams")
        .value("useTemperature", TestValue::Bool(true))
        .value("defaultState", TestValue::Struct(mode))
        .value(
            "fixtureSamples",
            TestValue::Array(vec![TestValue::UInt8(4)]),
        );
    let entity = TestEntity::new(
        CLASS,
        "SyntheticLight",
        "libs/foundry/records/entities/test/light",
    )
    .component(TestValue::Struct(component));
    entity_schema(DataForgeBuilder::new())
        .structure(
            TestStruct::new("FixtureLightMode")
                .field("intensity", TestType::Single)
                .field("temperature", TestType::Single)
                .field("lightStyle", TestType::UInt8),
        )
        .structure(
            TestStruct::new("LightComponentParams")
                .parent("DataForgeComponentParams")
                .field("useTemperature", TestType::Bool)
                .field(
                    "defaultState",
                    TestType::Class("FixtureLightMode".to_owned()),
                )
                .array("fixtureSamples", TestType::UInt8),
        )
        .record(entity.record())
        .build()
}

const LIGHTS: &[u8] = br#"<Entities>
<Entity EntityClassGUID="22222222-2222-2222-2222-222222222222"><PropertiesDataCore>
<EntityComponentLight useTemperature="false"><defaultState intensity="0" lightStyle="256"/></EntityComponentLight>
</PropertiesDataCore></Entity>
<Entity EntityClass="SyntheticLight"><PropertiesDataCore>
<EntityComponentLight><defaultState intensity="1"/><defaultState intensity="2"/></EntityComponentLight>
</PropertiesDataCore></Entity>
<Entity EntityClassGUID="11111111-1111-1111-1111-111111111111"><PropertiesDataCore>
<EntityComponentLight/>
</PropertiesDataCore></Entity>
<Entity EntityClassGUID="invalid" EntityClass="SyntheticLight"><PropertiesDataCore>
<EntityComponentLight/>
</PropertiesDataCore></Entity>
</Entities>"#;

fn source() -> TestResult<(Value, Value)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("light-class.p4k");
    let soc = TestChunkFile::crch().chunk(0x0004, 3, LIGHTS).bytes();
    let container = TestContainer::new()
        .entry(TestEntry::stored("fixture.xml", br#"<ObjectContainer><StreamingOCs><OC name="fixture.soc"/></StreamingOCs></ObjectContainer>"#))
        .entry(TestEntry::stored("fixture.soc", soc));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("fixture.socpak", container.build()))
        .entry(TestEntry::stored("Data/Game2.dcb", forge()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let container =
        archive.open_container(archive.entries().first().ok_or("fixture container")?)?;
    let cancel = CancelToken::new();
    let capture = SocContainerCapture::read(&[&archive, &container], &cancel)?;
    Ok((
        serde_json::to_value(SocScene::from_archive_capture(&capture, &archive, &cancel)?)?,
        serde_json::to_value(SocScene::from_capture(&capture)?)?,
    ))
}

fn field<'a>(light: &'a Value, path: &[&str]) -> TestResult<&'a Value> {
    light["fields"]
        .as_array()
        .ok_or("effective fields")?
        .iter()
        .find(|field| {
            field["path"].as_array().is_some_and(|steps| {
                steps
                    .iter()
                    .filter_map(Value::as_str)
                    .eq(path.iter().copied())
            })
        })
        .ok_or_else(|| "effective field".into())
}

fn lights(scene: &Value) -> TestResult<Vec<&Value>> {
    let entities = scene
        .pointer("/occurrences/0/streams/0/member/data/0/entities")
        .and_then(Value::as_array)
        .ok_or("authored entities")?;
    entities
        .iter()
        .map(|entity| {
            entity
                .pointer("/lights/0/effective")
                .ok_or_else(|| "light projection".into())
        })
        .collect()
}

#[test]
fn light_class_defaults_keep_override_states_and_both_sources() -> TestResult {
    let (scene, without_forge) = source()?;
    jsonschema::validator_for(&soc_scene_schema())?
        .validate(&scene)
        .map_err(|error| error.to_string())?;
    let [light, repeated, missing, invalid] =
        lights(&scene)?.try_into().map_err(|_| "four lights")?;
    assert_eq!(light["class"]["value"]["type"], "matched");
    assert_eq!(
        repeated["class"]["value"]["recordIndex"],
        light["class"]["value"]["recordIndex"]
    );
    assert_eq!(missing["class"]["value"]["type"], "missing");
    assert_eq!(invalid["class"]["state"], "invalid");
    assert_eq!(lights(&without_forge)?[0]["class"]["state"], "unreadable");

    let intensity = field(light, &["defaultState", "intensity"])?;
    assert_eq!(intensity["selection"], "authored");
    assert_eq!(intensity["value"]["value"]["value"]["bits"], 0);
    let enabled = field(light, &["useTemperature"])?;
    assert_eq!(enabled["selection"], "authored");
    assert_eq!(enabled["value"]["value"]["value"], false);
    let temperature = field(light, &["defaultState", "temperature"])?;
    assert_eq!(temperature["selection"], "inherited");
    assert_eq!(temperature["value"]["value"]["value"]["finite"], 4800.0);
    assert_eq!(temperature["value"]["containsEncryptedSource"], true);
    assert_eq!(temperature["value"]["sourceSetComplete"], true);
    assert_eq!(
        field(light, &["defaultState", "lightStyle"])?["value"]["state"],
        "invalid"
    );
    let conflict = field(repeated, &["defaultState", "intensity"])?;
    assert_eq!(conflict["selection"], "conflictingOverride");
    assert_eq!(conflict["value"]["state"], "invalid");
    Ok(())
}

#[test]
fn light_class_retains_positioned_array_leaves() -> TestResult {
    let (scene, _) = source()?;
    let classes = scene["lightClasses"].as_array().ok_or("light classes")?;
    assert_eq!(classes.len(), 1);
    let fields = classes[0]["components"]["value"][0]["fields"]
        .as_array()
        .ok_or("class fields")?;
    let sample = fields
        .iter()
        .find(|field| field["path"] == "fixtureSamples[0]")
        .ok_or("array leaf")?;
    assert_eq!(sample["value"]["value"]["value"], 4);
    assert_eq!(sample["value"]["containsEncryptedSource"], true);
    assert!(sample["ordinal"].is_u64());
    assert!(field(lights(&scene)?[0], &["fixtureSamples[0]"]).is_err());
    Ok(())
}
