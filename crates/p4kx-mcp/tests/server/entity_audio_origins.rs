// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestAtl, TestBank, TestEntity, TestEntry, TestHircObject, audio_schema,
    entity_schema, wwise_short_id,
};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::audio_origin_assertions as proof;
use crate::support::{Install, TestResult};

const TOOL: &str = "get_entity_sounds";
const ENTITY: &str = "TEST_Audio_Origins";
const RECORD: &str = "Data/Game2.dcb";
const ATL: &str = "Data/Libs/GameAudio/SC_Provenance.xml";
const BANK: &str = "Data/Sounds/wwise/English/Shared.bnk";
const EVENT: &str = "Play_Provenance_Start";
const TRIGGER: &str = "Play_Provenance";
const ROOT: &str = "/structuredContent";
const TRIGGER_ROW: &str = "/structuredContent/triggers/0";
const EVENT_ROW: &str = "/structuredContent/triggers/0/events/0";
const RESPONSE_LIMIT: usize = 256 * 1024;

#[test]
fn entity_audio_preserves_nested_trigger_spellings_and_physical_banks() -> TestResult {
    let record = database(
        &entity()
            .audio_trigger(TRIGGER)
            .audio_trigger("play_provenance")
            .sound_effect("nested", "PLAY_PROVENANCE"),
    );
    let definition = TestAtl::new("Provenance")
        .localised_trigger(TRIGGER, EVENT)
        .to_cryxml();
    let id = wwise_short_id(EVENT);
    let first = TestBank::new(77)
        .object(TestHircObject::event(id, &[]))
        .object(TestHircObject::event(id, &[]))
        .bytes();
    let second = TestBank::new(78)
        .object(TestHircObject::event(3, &[]))
        .object(TestHircObject::event(id, &[]))
        .bytes();
    let install = Install::new(vec![
        TestEntry::stored(RECORD, record.as_slice()),
        TestEntry::deflated(ATL, definition).encrypted(),
        TestEntry::stored(BANK, first.as_slice()),
        TestEntry::deflated(BANK, second.as_slice()).encrypted(),
    ])?;
    let resources = [&first[..], &second[..]];
    let mut server = serve(&install)?;
    let reply = server.ok(TOOL, &json!({"entity":ENTITY}))?;
    validate_receipt(&reply)?;
    assert_trigger(&reply, &record)?;
    assert_event(&reply, id)?;
    assert_bank_holders(&reply, id, resources)
}

fn entity() -> TestEntity {
    TestEntity::new(927, ENTITY, "entities/test_audio_origins.xml")
}

fn database(entity: &TestEntity) -> Vec<u8> {
    audio_schema(entity_schema(DataForgeBuilder::new()))
        .record(entity.record())
        .build()
}

fn assert_trigger(reply: &Value, record: &[u8]) -> TestResult {
    assert_eq!(
        reply.pointer("/entity/class_name"),
        Some(&json!(ENTITY)),
        "original Entity"
    );
    assert_eq!(
        reply.get("total"),
        Some(&json!(1)),
        "case aliases share one trigger"
    );
    assert_eq!(
        reply.get("matched"),
        Some(&json!(1)),
        "the trigger has banks"
    );
    assert_eq!(
        reply.pointer("/triggers/0/name"),
        Some(&json!(TRIGGER)),
        "first authored spelling"
    );
    assert_eq!(
        reply.pointer("/triggers/0/location"),
        Some(&json!("Components[0].triggers[0].audioTrigger")),
        "first original location"
    );
    assert_eq!(
        reply.pointer("/triggers/0/references"),
        Some(&json!(3)),
        "all physical occurrences"
    );
    proof::assert_marker(reply, &format!("{TRIGGER_ROW}/name"), false)?;
    proof::assert_marker(reply, &format!("{TRIGGER_ROW}/references"), false)?;
    assert_eq!(
        dataforge_spellings(reply, &format!("{TRIGGER_ROW}/name"), "payload", record)?,
        [TRIGGER.as_bytes()],
        "the name uses only the first spelling"
    );
    let mut spellings = dataforge_spellings(
        reply,
        &format!("{TRIGGER_ROW}/references"),
        "conversion",
        record,
    )?;
    spellings.sort_unstable();
    assert_eq!(
        spellings,
        [
            b"PLAY_PROVENANCE".as_slice(),
            TRIGGER.as_bytes(),
            b"play_provenance".as_slice()
        ],
        "the count retains every exact trigger string"
    );
    assert!(
        proof::uses(reply, &format!("{TRIGGER_ROW}/references"))?
            .into_iter()
            .any(|site| site.pointer("/location/field") == Some(&json!("pointerSelector"))),
        "the nested trigger retains its owning pointer"
    );
    Ok(())
}

fn dataforge_spellings<'a>(
    reply: &Value,
    field: &str,
    role: &str,
    record: &'a [u8],
) -> TestResult<Vec<&'a [u8]>> {
    let mut spellings = Vec::new();
    for site in proof::uses(reply, field)? {
        if !same_location(site, "dataforgeDecodedBytes", "text")
            || site.get("role") != Some(&json!(role))
        {
            continue;
        }
        assert_eq!(
            proof::source_path(reply, site)?,
            RECORD,
            "the original DCB owns trigger text"
        );
        spellings.push(source_bytes(site, record)?);
    }
    Ok(spellings)
}

fn assert_event(reply: &Value, id: u32) -> TestResult {
    assert_eq!(
        reply.pointer("/triggers/0/definition"),
        Some(&json!(ATL)),
        "the winning definition"
    );
    assert_eq!(
        reply.pointer("/triggers/0/events_total"),
        Some(&json!(1)),
        "one authored event"
    );
    assert_eq!(
        reply.pointer("/triggers/0/events/0/name"),
        Some(&json!(EVENT)),
        "the original event"
    );
    assert_eq!(
        reply.pointer("/triggers/0/events/0/id"),
        Some(&json!(id)),
        "the Wwise hash"
    );
    assert_eq!(
        reply.pointer("/triggers/0/events/0/localised"),
        Some(&json!(true)),
        "the authored flag"
    );
    for field in ["name", "id", "localised"] {
        proof::assert_marker(reply, &format!("{EVENT_ROW}/{field}"), true)?;
    }
    let mut conversion_owners = Vec::new();
    for site in proof::uses(reply, &format!("{EVENT_ROW}/id"))? {
        if site.get("role") == Some(&json!("conversion")) {
            conversion_owners.push(proof::source_path(reply, site)?);
        }
    }
    assert!(
        conversion_owners.contains(&ATL),
        "the generated Wwise ID cites conversion from its ATL Entry"
    );
    Ok(())
}

fn assert_bank_holders(reply: &Value, id: u32, resources: [&[u8]; 2]) -> TestResult {
    assert_eq!(
        reply.pointer("/triggers/0/events/0/soundbank_count"),
        Some(&json!(2)),
        "duplicate objects share their bank while physical path aliases count separately"
    );
    for row in 0..2 {
        let path = format!("/triggers/0/events/0/soundbanks/{row}/path");
        let field = format!("{ROOT}{path}");
        assert_eq!(
            reply.pointer(&path),
            Some(&json!(BANK)),
            "literal alias preserved"
        );
        proof::assert_marker(reply, &field, true)?;
        let source = proof::uses(reply, &field)?
            .into_iter()
            .find(|site| site.pointer("/location/field") == Some(&json!("path")))
            .ok_or("bank path use")?;
        assert_eq!(
            source.get("scope"),
            Some(&json!("archiveDirectoryMetadata")),
            "unprotected directory word"
        );
        assert_eq!(
            entry_index(reply, source)?,
            row + 2,
            "selected physical bank"
        );
    }
    let mut holders = Vec::new();
    for site in proof::uses(reply, &format!("{EVENT_ROW}/soundbank_count"))? {
        if same_location(site, "bnkHircObject", "id") {
            let index = entry_index(reply, site)?;
            let slot = index.checked_sub(2).ok_or("bank ordinal")?;
            let bytes = resources.get(slot).ok_or("bank payload")?;
            assert_eq!(
                source_bytes(site, bytes)?,
                id.to_le_bytes().as_slice(),
                "the HIRC ID word is exact"
            );
            holders.push(index);
        }
    }
    holders.sort_unstable();
    assert_eq!(
        holders,
        [2, 2, 3],
        "all three original HIRC declarations are retained"
    );
    Ok(())
}

#[test]
fn an_unreadable_audio_candidate_leaves_absence_unproved() -> TestResult {
    let dcb = database(&entity().audio_trigger("No_Indexed_Definition"));
    let install = Install::new(vec![
        TestEntry::stored(RECORD, dcb),
        TestEntry::deflated(ATL, b"partial XML".to_vec()).encrypted(),
    ])?;
    let mut server = serve(&install)?;
    let reply = server.ok(TOOL, &json!({"entity":ENTITY}))?;
    assert_eq!(
        reply.get("issues_total"),
        Some(&json!(1)),
        "the damaged candidate is counted"
    );
    assert_eq!(
        reply.pointer("/issues/0/path"),
        Some(&json!(ATL)),
        "failed physical definition"
    );
    assert_eq!(
        reply.pointer("/triggers/0/definition"),
        Some(&Value::Null),
        "no readable definition"
    );

    for field in [
        format!("{TRIGGER_ROW}/definition"),
        format!("{ROOT}/issues_total"),
    ] {
        let origin = proof::origin(&reply, &field)?;
        assert_eq!(
            origin.get("containsEncryptedSource"),
            Some(&Value::Null),
            "discarded encrypted input does not contribute to {field}"
        );
        assert_eq!(
            origin.get("sourceSetComplete"),
            Some(&json!(false)),
            "the evidence for {field} is incomplete"
        );
    }
    validate_receipt(&reply)
}

#[test]
fn entity_audio_pages_budget_the_receipt_and_keep_original_trigger_order() -> TestResult {
    const COUNT: usize = 4;
    let names: Vec<String> = (0..COUNT)
        .map(|ordinal| format!("Play_Page_{ordinal}_{}", "x".repeat(127 * 1024)))
        .collect();
    let entity = names.iter().fold(entity(), TestEntity::audio_trigger);
    let install = Install::new(vec![TestEntry::deflated(RECORD, database(&entity))])?;
    let mut server = serve(&install)?;
    let mut next = None::<String>;
    let mut returned = 0;
    loop {
        let page = server.ok(TOOL, &json!({"entity":ENTITY,"limit":COUNT,"cursor":next}))?;
        assert_eq!(
            page.get("total"),
            Some(&json!(COUNT)),
            "the count spans pages"
        );
        assert!(
            serde_json::to_vec(&page)?.len() <= RESPONSE_LIMIT,
            "the complete reply fits"
        );
        validate_receipt(&page)?;
        let rows = page
            .get("triggers")
            .and_then(Value::as_array)
            .ok_or("trigger page")?;
        assert!(!rows.is_empty(), "paging always makes progress");
        if returned == 0 {
            assert_eq!(
                rows.len(),
                1,
                "evidence uses the remaining space of a two-row page"
            );
        }
        for (position, row) in rows.iter().enumerate() {
            assert_eq!(
                row.get("name").and_then(Value::as_str),
                names.get(returned).map(String::as_str),
                "next original trigger"
            );
            proof::assert_marker(&page, &format!("{ROOT}/triggers/{position}/name"), false)?;
            returned += 1;
        }
        next = page
            .get("next_cursor")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if next.is_none() {
            break;
        }
        assert!(returned < COUNT, "a cursor requires another trigger");
    }
    assert_eq!(returned, COUNT, "the budget loses no rows");
    Ok(())
}

fn validate_receipt(reply: &Value) -> TestResult {
    let fields = reply
        .get("fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("field origins")?;
    assert!(!fields.is_empty(), "the endpoint emits provenance");
    for address in fields.keys() {
        let field = address
            .strip_prefix(ROOT)
            .ok_or("fully rooted origin pointer")?;
        assert!(
            reply.pointer(field).is_some(),
            "{address} resolves in this page"
        );
        for usage in proof::uses(reply, address)? {
            proof::physical_source(reply, usage)?;
        }
    }
    Ok(())
}

fn same_location(site: &Value, namespace: &str, field: &str) -> bool {
    site.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
        && site.pointer("/location/field").and_then(Value::as_str) == Some(field)
}

fn source_bytes<'a>(site: &Value, original: &'a [u8]) -> TestResult<&'a [u8]> {
    let offset = |path: &str| -> TestResult<usize> {
        Ok(site
            .pointer(path)
            .and_then(Value::as_str)
            .ok_or("byte offset")?
            .parse()?)
    };
    original
        .get(offset("/location/indices/0")?..offset("/location/indices/1")?)
        .ok_or_else(|| "valid decoded-byte span".into())
}

fn entry_index(reply: &Value, use_site: &Value) -> TestResult<usize> {
    let source = proof::physical_source(reply, use_site)?;
    Ok(usize::try_from(
        source
            .get("entryIndex")
            .and_then(Value::as_u64)
            .ok_or("entry index")?,
    )?)
}
