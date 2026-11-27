// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAtl, TestBank, TestEntity, TestEntry, TestHircObject,
    audio_schema, entity_schema, wwise_short_id,
};
use serde_json::{Value, json};

use super::{PLAY, TestResult};

const TRIGGER: &str = "Play_Origin";
const ALIAS: &str = "pLAY_oRIGIN";
const EVENT: &str = "Play_Origin_Event";
const LOCATION: &str = "Data\\Sounds\\wwise\\Origin.bnk";
const WEM: u32 = 5000;

#[test]
fn json_links_entity_occurrences_and_resolved_variants_in_one_origin_inventory() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let entity = TestEntity::new(
        1,
        "TEST_Origin",
        "libs/foundry/records/entities/test_origin.xml",
    )
    .audio_trigger(TRIGGER)
    .sound_effect("duplicate spelling", ALIAS)
    .audio_trigger("Play_No_Origin_Definition");
    let dataforge = audio_schema(entity_schema(DataForgeBuilder::new()))
        .record(entity.record())
        .build();
    let atl = TestAtl::new("SC_Origin").trigger(TRIGGER, &[EVENT]);
    let bank = TestBank::new(wwise_short_id("Origin"))
        .object(TestHircObject::event(wwise_short_id(EVENT), &[1]))
        .object(TestHircObject::action(1, PLAY, 2))
        .object(TestHircObject::sound(2, WEM, 0, 4))
        .wem(WEM, vec![5; 4])
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", dataforge.clone()).encrypted())
        .entry(TestEntry::stored(
            "Data\\Libs\\GameAudio\\SC_Origin.xml",
            atl.to_xml(),
        ))
        .entry(TestEntry::stored(LOCATION, bank.clone()))
        .entry(TestEntry::zstd(LOCATION, bank).encrypted())
        .write_to(&path)?;
    let result = p4kx_testkit::p4kx()?
        .env("P4KX_ARCHIVE_KEY", p4kx_testkit::test_key().to_hex())
        .args(["--json", "entity", "audio"])
        .arg(path)
        .args(["TEST_Origin", "--sounds"])
        .output()?;
    assert!(
        result.status.success(),
        "synthetic Entity resolves: {result:?}"
    );
    let value: Value = serde_json::from_slice(&result.stdout)?;
    verify_sources(&value, &dataforge)?;
    verify_resolution(&value)?;
    verify_inventory(&value)
}

fn verify_sources(value: &Value, dataforge: &[u8]) -> TestResult {
    assert_eq!(
        value.pointer("/triggers/0/name"),
        Some(&json!(TRIGGER)),
        "keeps first original spelling"
    );
    assert_eq!(
        value.pointer("/triggers/0/references"),
        Some(&json!(2)),
        "both original occurrences count"
    );
    let receipt = origin(value, "/triggers/0/name")?;
    assert_eq!(
        receipt["containsEncryptedSource"], true,
        "original Entity payload is encrypted"
    );
    assert_eq!(
        receipt["sourceSetComplete"], true,
        "the chosen trigger spelling has its coordinates"
    );

    let mut names = Vec::new();
    for site in field_uses(value, "/triggers/0/references")? {
        if site.pointer("/location/namespace").and_then(Value::as_str)
            == Some("dataforgeDecodedBytes")
            && site.pointer("/location/field").and_then(Value::as_str) == Some("text")
            && site["role"] == "conversion"
        {
            let [start, end, ..] = array(site, "/location/indices")? else {
                return Err("text span".into());
            };
            let index = |value: &Value| -> Result<usize, Box<dyn Error>> {
                Ok(value.as_str().ok_or("byte coordinate")?.parse()?)
            };
            names.push(
                dataforge
                    .get(index(start)?..index(end)?)
                    .ok_or("decoded original name")?,
            );
        }
    }
    names.sort_unstable();
    assert_eq!(
        names,
        [TRIGGER.as_bytes(), ALIAS.as_bytes()],
        "both exact original spellings are retained"
    );
    verify_unmatched(value)
}

fn verify_unmatched(value: &Value) -> TestResult {
    assert_eq!(
        value.pointer("/triggers/1/definition"),
        Some(&Value::Null),
        "the second trigger stays unresolved"
    );
    assert_eq!(
        value.pointer("/triggers/1/unmatched/kind"),
        Some(&json!("no_definition")),
        "the absence is explicit"
    );
    let missing = origin(value, "/triggers/1/unmatched/kind")?;
    assert_eq!(
        missing["sourceSetComplete"], true,
        "the absence retains a complete definition survey"
    );
    Ok(())
}

fn verify_resolution(value: &Value) -> TestResult {
    let indices = [0, 1].map(|ordinal| {
        let path = format!("/triggers/0/events/0/soundbanks/{ordinal}/path");
        holder_index(value, &path)
    });
    let [first, second] = indices;
    let mut indices = [first?, second?];
    indices.sort_unstable();
    assert_eq!(
        indices,
        [2, 3],
        "the same spelling keeps both physical soundbank owners"
    );

    let variants = array(value, "/triggers/0/sounds/variants")?;
    assert!(
        !variants.is_empty(),
        "the selected trigger emitted its resolved variants"
    );
    let nested = field_uses(value, "/triggers/0/sounds/variants/0/location/len")?;
    for (namespace, field) in [("dataforgeDecodedBytes", "text"), ("bnkDidx", "dataLength")] {
        assert!(
            nested.iter().any(|source| {
                source
                    .pointer("/location/namespace")
                    .and_then(Value::as_str)
                    == Some(namespace)
                    && source.pointer("/location/field").and_then(Value::as_str) == Some(field)
            }),
            "nested length keeps actual {namespace}.{field} input"
        );
    }
    assert!(
        value.pointer("/triggers/0/sounds/provenance").is_none(),
        "nested fields share the outer inventory"
    );
    Ok(())
}

fn holder_index(value: &Value, path: &str) -> Result<u64, Box<dyn Error>> {
    let uses = field_uses(value, path)?;
    let declaration = uses
        .iter()
        .find(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str) == Some("bnkHircObject")
                && site.pointer("/location/field").and_then(Value::as_str) == Some("id")
        })
        .ok_or("original HIRC Event holder")?;
    let source = array(value, "/provenance/sources")?
        .iter()
        .find(|source| source["id"] == declaration["source"])
        .ok_or("HIRC physical Entry")?;
    source["entryIndex"]
        .as_u64()
        .ok_or_else(|| "original holder ordinal".into())
}

fn verify_inventory(value: &Value) -> TestResult {
    let fields = value
        .get("fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("field origins")?;
    for address in fields.keys() {
        assert!(
            value.pointer(address).is_some(),
            "{address} addresses a real field"
        );
        field_uses(value, address)?;
    }
    Ok(())
}

fn field_uses<'a>(value: &'a Value, address: &str) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let references = array(origin(value, address)?, "/sourceUsesRefs")?;
    let uses = array(value, "/provenance/uses")?;
    references
        .iter()
        .map(|reference| {
            uses.iter()
                .find(|site| site.get("id") == Some(reference))
                .ok_or_else(|| format!("unresolved use at {address}").into())
        })
        .collect()
}

fn origin<'a>(value: &'a Value, address: &str) -> Result<&'a Value, Box<dyn Error>> {
    value
        .get("fieldOrigins")
        .and_then(|fields| fields.get(address))
        .ok_or_else(|| format!("missing origin at {address}").into())
}

fn array<'a>(value: &'a Value, path: &str) -> Result<&'a [Value], Box<dyn Error>> {
    value
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("expected array at {path}").into())
}
