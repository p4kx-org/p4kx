// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::SocContainerCapture;
use p4kx_blender::{SocScene, soc_scene_schema};
use p4kx_extraction::{Archive, CancelToken};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestChunkFile, TestContainer, TestElement,
    TestEntry,
};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const CHUNK: &str = "/occurrences/0/streams/0/member/data/0";
const TYPE: &str = "FogVolumeComponentParams";
const TEXT_FOG: &[u8] = br#"<Entities xmlns:wrong="urn:fixture"><Entity>
<PropertiesDataCore>
<Other><FogVolume __type="FogVolumeComponentParams"/></Other>
<wrong:FogVolume __type="FogVolumeComponentParams"/>
<FogVolume __type="Unrecognized"/>
<FogVolume wrong:__type="FogVolumeComponentParams"/>
<FogVolume __type="FogVolumeComponentParams" volumeType="7" globalDensity=".25" softEdges=".75" rampStart="2" rampEnd="3" rampInfluence=".5" affectsThisAreaOnly="0" maxDistance="40" otherField="Uninterpreted">
<size __type="Vec3" x="1" y="-0.0" z="3"/>
<size __type="Unknown" x="9"/>
<size __type="Vec3" x="5"/>
</FogVolume>
<FogVolume __type="FogVolumeComponentParams"/>
</PropertiesDataCore>
<FogVolume FogVolumeHash="4294967297"/>
</Entity></Entities>"#;

#[test]
fn fog_rows_admit_only_the_authored_component_and_vec3_sizes() -> TestResult {
    let scene = capture(TEXT_FOG, false)?;
    let chunk = scene.pointer(CHUNK).ok_or("SOC entity chunk")?;
    let entity = &chunk["entities"][0];
    let components = entity["components"].as_array().ok_or("entity components")?;
    let [first, second] = components.as_slice() else {
        return Err("expected two authored fog components".into());
    };
    assert_eq!(first["ordinal"], json!([0, 4]), "original component slot");
    assert_eq!(second["ordinal"], json!([0, 5]), "following component slot");
    assert_eq!(first["kind"], "fogVolume");
    assert_eq!(first["declaredType"]["value"], TYPE);
    let fields = &first["fogVolume"];
    for (name, value) in [
        ("volumeType", json!(7)),
        ("globalDensity", json!(0.25)),
        ("softEdges", json!(0.75)),
        ("rampStart", json!(2.0)),
        ("rampEnd", json!(3.0)),
        ("rampInfluence", json!(0.5)),
        ("affectsThisAreaOnly", json!(false)),
        ("maxDistance", json!(40.0)),
    ] {
        assert_eq!(fields[name]["value"], value, "{name}");
        assert_eq!(fields[name]["state"], "present", "{name}");
    }
    let sizes = fields["sizes"].as_array().ok_or("Vec3 sizes")?;
    let [first_size, last_size] = sizes.as_slice() else {
        return Err("expected both ordered Vec3 sizes".into());
    };
    assert_eq!(first_size["x"]["value"], 1.0);
    assert_eq!(last_size["x"]["value"], 5.0);
    assert_eq!(last_size["z"]["state"], "absent");
    assert_eq!(second["fogVolume"]["globalDensity"]["state"], "absent");
    assert_eq!(second["fogVolume"]["sizes"], json!([]));
    verify_text_density_origin(&scene, fields)?;
    let nodes = nodes(&scene, chunk)?;
    assert!(nodes.iter().any(
        |node| node["attributes"].as_array().is_some_and(|attributes| {
            attributes
                .iter()
                .any(|attr| attr["name"] == "FogVolumeHash" && attr["value"] == "4294967297")
        })
    ));
    Ok(())
}

#[test]
fn fog_fields_keep_original_cryxml_coordinates_and_repeated_values() -> TestResult {
    let xml = cryxml_fog_fixture();
    let scene = capture(&xml, true)?;
    let chunk = scene.pointer(CHUNK).ok_or("SOC entity chunk")?;
    let components = chunk["entities"][0]["components"]
        .as_array()
        .ok_or("entity components")?;
    let [component] = components.as_slice() else {
        return Err("repeated component type must stay in the original XML".into());
    };
    let raw = nodes(&scene, chunk)?;
    let original = raw
        .iter()
        .find(|node| node["name"] == "FogVolume")
        .ok_or("original component")?;
    assert_eq!(component["position"], original["position"]);
    let fields = &component["fogVolume"];
    assert_eq!(fields["rampStart"]["state"], "invalid");
    assert_eq!(fields["maxDistance"]["state"], "invalid");
    assert_eq!(fields["rampEnd"]["state"], "absent");
    assert_eq!(fields["rampStart"]["sourceSetComplete"], true);
    assert_eq!(fields["rampStart"]["containsEncryptedSource"], true);
    let attrs = original["attributes"]
        .as_array()
        .ok_or("original attributes")?;
    let uses = scene["sourceInventory"]["uses"]
        .as_array()
        .ok_or("source uses")?;
    verify_repeated_ramp_origins(fields, uses, attrs)?;
    let sizes = fields["sizes"].as_array().ok_or("Vec3 sizes")?;
    let [size] = sizes.as_slice() else {
        return Err("repeated size type must stay in the original XML".into());
    };
    assert_eq!(size["x"]["value"], 3.0);
    assert_eq!(size["y"]["state"], "invalid");
    assert_eq!(size["z"]["state"], "absent");
    let properties = usize::try_from(original["parent"].as_u64().ok_or("property node")?)?;
    let property_node = raw.get(properties).ok_or("selected property node")?;
    let component_selectors = [
        ("$element", &property_node["position"]),
        ("$element", &component["position"]),
        ("__type", &attrs[0]["position"]),
    ];
    check_selectors(&fields["rampStart"], uses, &component_selectors)?;
    check_selectors(&fields["rampEnd"], uses, &component_selectors)?;
    let raw_size = raw
        .iter()
        .find(|node| node["position"] == size["position"])
        .ok_or("original size")?;
    let size_selectors = [
        component_selectors[0],
        component_selectors[1],
        component_selectors[2],
        ("$element", &size["position"]),
        ("__type", &raw_size["attributes"][0]["position"]),
    ];
    check_selectors(&size["x"], uses, &size_selectors)?;
    check_selectors(&size["z"], uses, &size_selectors)?;
    Ok(())
}

fn verify_text_density_origin(scene: &Value, fields: &Value) -> TestResult {
    assert_eq!(
        fields.pointer("/globalDensity/containsEncryptedSource"),
        Some(&Value::Null),
        "unknown archive encryption remains explicit"
    );
    let refs = fields
        .pointer("/globalDensity/sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("density origin uses")?;
    let uses = scene
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let location = uses
        .iter()
        .filter(|usage| usage.get("id").is_some_and(|id| refs.contains(id)))
        .filter_map(|usage| usage.get("location"))
        .find(|location| location.get("field").and_then(Value::as_str) == Some("globalDensity"))
        .ok_or("density origin coordinate")?;
    assert_eq!(
        location.get("namespace").and_then(Value::as_str),
        Some("socChunk.text"),
        "density keeps its original text coordinate"
    );
    let indices = location
        .get("indices")
        .and_then(Value::as_array)
        .ok_or("source byte coordinates")?;
    let [_, start, end] = indices.as_slice() else {
        return Err("expected one chunk and one text range".into());
    };
    let start = start.as_str().ok_or("start byte")?.parse()?;
    let end = end.as_str().ok_or("end byte")?.parse()?;
    assert_eq!(
        TEXT_FOG.get(start..end),
        Some(br#"globalDensity=".25""#.as_slice()),
        "source range covers the authored density attribute"
    );
    Ok(())
}

fn cryxml_fog_fixture() -> Vec<u8> {
    let component = TestElement::new("FogVolume")
        .attribute("__type", TYPE)
        .attribute("rampStart", "-0.0")
        .attribute("rampStart", "2")
        .attribute("maxDistance", "NaN")
        .child(
            TestElement::new("size")
                .attribute("__type", "Vec3")
                .attribute("x", "3")
                .attribute("y", "broken"),
        )
        .child(
            TestElement::new("size")
                .attribute("__type", "Vec3")
                .attribute("__type", "Vec3"),
        );
    let rejected = TestElement::new("FogVolume")
        .attribute("__type", TYPE)
        .attribute("__type", TYPE);
    TestElement::new("Entities")
        .child(
            TestElement::new("Entity").child(
                TestElement::new("PropertiesDataCore")
                    .child(component)
                    .child(rejected),
            ),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn verify_repeated_ramp_origins(fields: &Value, uses: &[Value], attrs: &[Value]) -> TestResult {
    let [_, first_ramp, second_ramp, ..] = attrs else {
        return Err("original repeat attribute positions".into());
    };
    let refs = fields
        .pointer("/rampStart/sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("ramp origins")?;
    let positions: Vec<_> = uses
        .iter()
        .filter(|usage| usage.get("id").is_some_and(|id| refs.contains(id)))
        .filter_map(|usage| usage.get("location"))
        .filter(|location| location.get("field").and_then(Value::as_str) == Some("rampStart"))
        .collect();
    assert_eq!(positions.len(), 2, "both repeated positions are retained");
    for (origin, attr) in positions.iter().zip([first_ramp, second_ramp]) {
        let node = attr
            .pointer("/position/node")
            .and_then(Value::as_u64)
            .ok_or("original node")?;
        let attribute = attr
            .pointer("/position/attribute")
            .and_then(Value::as_u64)
            .ok_or("original attribute")?;
        assert_eq!(
            origin.get("namespace").and_then(Value::as_str),
            Some("socChunk.cryXml"),
            "repeat is bound to the original CryXml coordinate"
        );
        assert_eq!(
            origin.get("indices"),
            Some(&json!(["0", node.to_string(), attribute.to_string()])),
            "each attribute keeps its original position"
        );
    }
    assert_eq!(
        first_ramp.get("value").and_then(Value::as_str),
        Some("-0.0"),
        "first lexical value"
    );
    assert_eq!(
        second_ramp.get("value").and_then(Value::as_str),
        Some("2"),
        "second lexical value"
    );
    Ok(())
}

fn check_selectors(field: &Value, uses: &[Value], expected: &[(&str, &Value)]) -> TestResult {
    let refs = field["sourceUsesRefs"].as_array().ok_or("field origins")?;
    let selectors: Vec<_> = uses
        .iter()
        .filter(|usage| refs.contains(&usage["id"]) && usage["role"] == "selector")
        .map(|usage| &usage["location"])
        .collect();
    assert_eq!(
        selectors.len(),
        expected.len(),
        "distinct shape selector count"
    );
    for (name, position) in expected {
        let node = position["node"].as_u64().ok_or("selector node")?;
        let mut indices = vec!["0".to_owned(), node.to_string()];
        if let Some(attribute) = position["attribute"].as_u64() {
            indices.push(attribute.to_string());
        }
        assert!(
            selectors.iter().any(|location| {
                location["namespace"] == "socChunk.cryXml"
                    && location["field"] == *name
                    && location["indices"] == json!(indices)
            }),
            "{name} selector at original coordinate {indices:?}"
        );
    }
    Ok(())
}

fn nodes<'document>(scene: &'document Value, chunk: &Value) -> TestResult<&'document [Value]> {
    let document = chunk["xml"].as_u64().ok_or("XML document index")?;
    let index = usize::try_from(document)?;
    scene
        .get("documents")
        .and_then(Value::as_array)
        .and_then(|docs| docs.get(index))
        .and_then(|document| document.get("nodes"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "document nodes".into())
}

fn capture(xml: &[u8], encrypted: bool) -> TestResult<Value> {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("fog.p4k");
    let descriptor =
        br#"<ObjectContainer><StreamingOCs><OC name="fog.soc"/></StreamingOCs></ObjectContainer>"#;
    let soc = TestChunkFile::crch().chunk(0x0004, 3, xml).bytes();
    let members = TestContainer::new()
        .entry(TestEntry::stored("fixture.xml", descriptor))
        .entry(TestEntry::stored("fog.soc", soc));
    let entry = TestEntry::stored("fixture.socpak", members.build());
    let entry = if encrypted { entry.encrypted() } else { entry };
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = archive.entries().first().ok_or("fixture container")?;
    let container = archive.open_container(entry)?;
    let capture = SocContainerCapture::read(&[&archive, &container], &CancelToken::new())?;
    let value = serde_json::to_value(SocScene::from_capture(&capture)?)?;
    jsonschema::validator_for(&soc_scene_schema())?
        .validate(&value)
        .map_err(|error| error.to_string())?;
    Ok(value)
}
