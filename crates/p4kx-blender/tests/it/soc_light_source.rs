// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::SocContainerCapture;
use p4kx_blender::{SocScene, soc_scene_schema};
use p4kx_extraction::{Archive, CancelToken};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestContainer, TestEntry};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const ENTITIES: &[u8] = br#"<Entities><Entity>
<PropertiesDataCore><EntityComponentLight lightType="FixturePlanar" useTemperature="false">
<emergencyState intensity="9"/><defaultState intensity="0" lightStyle="0"><color r="0"/></defaultState>
<defaultState intensity="wrong"/><auxiliaryState temperature="NaN"/><cinematicState/>
</EntityComponentLight></PropertiesDataCore>
<TileAttachmentParams ItemPortName="matched[tip]" ItemPortIndex="0"/><TileAttachmentParams ItemPortName="duplicate"/><TileAttachmentParams ItemPortName="missing"/><TileAttachmentParams/><TileAttachmentParams ItemPortName=""/><EntityComponentLightGroup helper="fixture_helper"><BakedInLights/><BakedInLights><Light>
<RelativeXForm rotation="1,0,0,0" translation="1,2,3"/>
<EntityComponentLight lightType="Omni"><defaultState intensity="1"/></EntityComponentLight>
</Light></BakedInLights></EntityComponentLightGroup>
</Entity></Entities>"#;

fn scene() -> TestResult<Value> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("authored-lights.p4k");
    let descriptor = br#"<ObjectContainer><StreamingOCs><OC name="lights.soc"/></StreamingOCs>
<TileXmlEntry><TileItemPortEntries><ItemPort name="matched[tip]" interactionOffset="1,2,3"/>
<ItemPort name="duplicate" interactionOffset="4,5,6"/><ItemPort name="duplicate"/>
</TileItemPortEntries></TileXmlEntry></ObjectContainer>"#;
    let soc = TestChunkFile::crch().chunk(0x0004, 3, ENTITIES).bytes();
    let container = TestContainer::new()
        .entry(TestEntry::stored("fixture.xml", descriptor))
        .entry(TestEntry::stored("lights.soc", soc));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("fixture.socpak", container.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let container =
        archive.open_container(archive.entries().first().ok_or("fixture container")?)?;
    let capture = SocContainerCapture::read(&[&archive, &container], &CancelToken::new())?;
    Ok(serde_json::to_value(SocScene::from_capture(&capture)?)?)
}

#[test]
fn light_rows_keep_source_modes_and_empty_group_sections() -> TestResult {
    let scene = scene()?;
    let schema = soc_scene_schema();
    jsonschema::validator_for(&schema)?
        .validate(&scene)
        .map_err(|error| error.to_string())?;
    let entity = scene
        .pointer("/occurrences/0/streams/0/member/data/0/entities/0")
        .ok_or("authored entity")?;
    let light = &entity["lights"][0];
    assert_eq!(
        light["lightType"]["value"], "FixturePlanar",
        "authored type"
    );
    assert_eq!(light["useTemperature"]["value"], false, "authored false");
    let modes = light["modes"].as_array().ok_or("ordered modes")?;
    assert!(
        modes.iter().filter_map(|mode| mode["kind"].as_str()).eq([
            "emergency",
            "default",
            "default",
            "auxiliary",
            "cinematic"
        ]),
        "order and duplicate modes survive"
    );
    verify_mode_evidence(modes)?;
    let group = &entity["lightGroups"][0];
    assert_eq!(
        group["helper"]["value"], "fixture_helper",
        "authored helper"
    );
    assert_eq!(
        group["helper"]["containsEncryptedSource"], true,
        "helper source marker"
    );
    assert_eq!(
        group["bakedSections"].as_array().map(Vec::len),
        Some(2),
        "empty section retained"
    );
    assert_eq!(
        group["lights"].as_array().map(Vec::len),
        Some(1),
        "only authored baked light"
    );
    let baked = &group["lights"][0];
    assert_eq!(
        baked["relativeFrames"].as_array().map(Vec::len),
        Some(1),
        "local frame coordinate"
    );
    assert_eq!(
        baked["components"][0]["lightType"]["value"], "Omni",
        "original group member"
    );
    Ok(())
}

fn verify_mode_evidence(modes: &[Value]) -> TestResult {
    let [_, default, invalid, auxiliary, cinematic] = modes else {
        return Err("five mode declarations".into());
    };
    let intensity = &default["intensity"];
    assert_eq!(intensity["value"].as_f64(), Some(0.0), "explicit zero");
    assert_eq!(
        intensity["containsEncryptedSource"], true,
        "value's original encrypted ancestry"
    );
    assert_eq!(
        intensity["sourceSetComplete"], true,
        "complete original field identity"
    );
    assert!(
        intensity["sourceUsesRefs"]
            .as_array()
            .is_some_and(|refs| refs.len() == 2),
        "node and attribute evidence"
    );
    assert_eq!(
        invalid.pointer("/intensity/state").and_then(Value::as_str),
        Some("invalid"),
        "malformed override survives"
    );
    assert_eq!(
        auxiliary
            .pointer("/temperature/state")
            .and_then(Value::as_str),
        Some("invalid"),
        "nonfinite temperature"
    );
    assert_eq!(
        cinematic
            .pointer("/intensity/state")
            .and_then(Value::as_str),
        Some("absent"),
        "missing mode field"
    );
    Ok(())
}

#[test]
fn tile_attachments_bind_exact_offsets_and_preserve_conflicts() -> TestResult {
    let scene = scene()?;
    let attachments = scene
        .pointer("/occurrences/0/streams/0/member/data/0/entities/0/tileAttachments")
        .and_then(Value::as_array)
        .ok_or("authored tile attachments")?;
    let [matched, duplicate, missing, absent, invalid] = attachments.as_slice() else {
        return Err("original attachment count".into());
    };
    verify_selector_fields(matched);
    let offset = matched
        .pointer("/target/value/port/interactionOffset")
        .ok_or("matched offset")?;
    verify_target_dispositions(offset, duplicate, missing, absent, invalid);
    Ok(())
}

fn verify_selector_fields(matched: &Value) {
    assert_eq!(
        matched.pointer("/portIndex/value").and_then(Value::as_str),
        Some("0"),
        "authored zero port index"
    );
    assert_eq!(
        matched
            .pointer("/attachmentPointSuffix/value")
            .and_then(Value::as_str),
        Some("tip"),
        "exact bracket suffix"
    );
    assert_eq!(
        matched
            .pointer("/attachmentPointSuffix/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "selector source marker"
    );
}

fn verify_target_dispositions(
    offset: &Value,
    duplicate: &Value,
    missing: &Value,
    absent: &Value,
    invalid: &Value,
) {
    assert_eq!(
        offset["value"],
        serde_json::json!([1.0, 2.0, 3.0]),
        "original owner offset"
    );
    assert_eq!(offset["containsEncryptedSource"], true, "offset provenance");
    assert_eq!(
        offset["sourceUsesRefs"].as_array().map(Vec::len),
        Some(2),
        "physical node and offset attribute"
    );
    assert_eq!(
        duplicate
            .pointer("/target/value/type")
            .and_then(Value::as_str),
        Some("ambiguous"),
        "duplicate target disposition"
    );
    assert_eq!(
        duplicate
            .pointer("/target/value/ports/1/interactionOffset/state")
            .and_then(Value::as_str),
        Some("absent"),
        "duplicate port without offset"
    );
    assert_eq!(
        missing
            .pointer("/target/value/type")
            .and_then(Value::as_str),
        Some("unmatched"),
        "no matching original port"
    );
    assert_eq!(
        absent.pointer("/target/state").and_then(Value::as_str),
        Some("absent"),
        "omitted selector"
    );
    assert_eq!(
        invalid.pointer("/target/state").and_then(Value::as_str),
        Some("invalid"),
        "invalid selector"
    );
}
