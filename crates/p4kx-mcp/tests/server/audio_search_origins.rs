// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestAtl, TestBank, TestEntity, TestEntry, TestHircObject, audio_schema,
    entity_schema, wwise_short_id,
};
use serde_json::{Value, json};

use crate::audio;
use crate::audio_origin_assertions as proof;
use crate::support::{Install, TestResult};

const TOOL: &str = "search_audio_triggers";
const ROOT: &str = "/structuredContent";
const ROW: &str = "/structuredContent/triggers/0";
const ATL: &str = "Data/Libs/GameAudio/SC_Cabin.xml";
const OTHER_ATL: &str = "Data/Libs/GameAudio/SC_Extra.xml";
const BANK: &str = "Data/Sounds/wwise/English/Beacon.bnk";
const DATABASE: &str = "Data/Game2.dcb";
const TRIGGER: &str = "Cabin_Warning";
const EVENT: &str = "Emit_Beacon";
const SIBLING_EVENT: &str = "Emit_Glass";
const OTHER_TRIGGER: &str = "Sleep_Cockpit";
const FIRST_ENTITY: &str = "TEST_Beacon_First";
const SECOND_ENTITY: &str = "TEST_Beacon_Second";
const FIRST_REFERENCE: &str = "cabin_warning";
const SECOND_REFERENCE: &str = "CABIN_WARNING";

struct Fixture {
    install: Install,
    atl: String,
    bank: Vec<u8>,
    database: Vec<u8>,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let first = TestEntity::new(928, FIRST_ENTITY, "entities/beacon_first.xml")
            .sound_effect("first", FIRST_REFERENCE);
        let second = TestEntity::new(929, SECOND_ENTITY, "entities/beacon_second.xml")
            .audio_trigger(SECOND_REFERENCE);
        let database = audio_schema(entity_schema(DataForgeBuilder::new()))
            .record(first.record())
            .record(second.record())
            .build();
        let atl = TestAtl::new("Cabin")
            .trigger(TRIGGER, &[EVENT, SIBLING_EVENT])
            .to_xml();
        let other = TestAtl::new("Extra").trigger(OTHER_TRIGGER, &[]).to_xml();
        let bank = TestBank::new(17)
            .object(TestHircObject::event(wwise_short_id(EVENT), &[]))
            .bytes();
        let install = Install::new(vec![
            TestEntry::deflated(DATABASE, database.as_slice()).encrypted(),
            TestEntry::stored(ATL, atl.as_bytes()),
            TestEntry::deflated(OTHER_ATL, other.into_bytes()).encrypted(),
            TestEntry::deflated(BANK, bank.as_slice()).encrypted(),
        ])?;
        Ok(Self {
            install,
            atl,
            bank,
            database,
        })
    }
}

#[test]
fn audio_search_records_the_actual_event_and_trigger_filter_operands() -> TestResult {
    let fixture = Fixture::new()?;
    let mut server = audio::serve(&fixture.install)?;
    let reply = server.ok(TOOL, &json!({"search": "bEaCoN", "entities": 0}))?;
    assert_eq!(
        reply.get("total"),
        Some(&json!(1)),
        "only one trigger matches"
    );
    let row = reply
        .pointer("/triggers/0")
        .ok_or("event-only trigger hit")?;
    assert_eq!(
        row.get("name"),
        Some(&json!(TRIGGER)),
        "the owner of the matching event"
    );
    assert_eq!(
        row.get("definition"),
        Some(&json!(ATL)),
        "the exact defining Entry"
    );

    assert_eq!(
        row.get("entities"),
        Some(&Value::Null),
        "zero requests no reverse Entity lookup"
    );

    for (member, matched, attribute, authored) in [
        ("matched_trigger", false, "atl_name", TRIGGER),
        ("events/0/matched", true, "wwise_name", EVENT),
        ("events/1/matched", false, "wwise_name", SIBLING_EVENT),
    ] {
        assert_eq!(
            row.pointer(&format!("/{member}")),
            Some(&json!(matched)),
            "filter decision at {member}"
        );
        let field = format!("{ROW}/{member}");
        proof::assert_marker(&reply, &field, false)?;
        assert_atl_operand(&reply, &field, attribute, authored, &fixture.atl)?;
    }
    for member in [
        "name",
        "definition",
        "events/0/name",
        "events/0/id",
        "events/1/name",
    ] {
        proof::assert_marker(&reply, &format!("{ROW}/{member}"), false)?;
    }
    assert_bank_join(&reply, &fixture.bank)?;
    assert_no_entity_uses(&reply)
}

fn assert_atl_operand(
    reply: &Value,
    field: &str,
    attribute: &str,
    authored: &str,
    xml: &str,
) -> TestResult {
    let expected = format!(r#"{attribute}="{authored}""#);
    let sites = proof::uses(reply, field)?;
    let operand = sites
        .into_iter()
        .find(|site| {
            site["role"] == "conversion"
                && site.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("textXmlAttribute")
                && site.pointer("/location/field").and_then(Value::as_str) == Some(attribute)
                && span(site, xml.as_bytes()).is_ok_and(|bytes| bytes == expected.as_bytes())
        })
        .ok_or_else(|| format!("{field} has no exact {attribute} filter operand"))?;
    assert_eq!(
        proof::source_path(reply, operand)?,
        ATL,
        "the operand belongs to the actual ATL Entry"
    );
    Ok(())
}

fn assert_bank_join(reply: &Value, bank: &[u8]) -> TestResult {
    let event = reply
        .pointer("/triggers/0/events/0")
        .ok_or("the matching event")?;
    assert_eq!(
        event.get("soundbank_count"),
        Some(&json!(1)),
        "one bank holds the event"
    );
    assert_eq!(
        event.pointer("/soundbanks/0/path"),
        Some(&json!(BANK)),
        "literal path of the owning bank"
    );
    let field = format!("{ROW}/events/0/soundbanks/0/path");
    proof::assert_marker(reply, &field, true)?;
    let sites = proof::uses(reply, &field)?;
    let id_site = sites
        .iter()
        .find(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str) == Some("bnkHircObject")
                && site.pointer("/location/field").and_then(Value::as_str) == Some("id")
        })
        .ok_or("a physical HIRC event must choose the bank")?;
    assert_eq!(
        proof::source_path(reply, id_site)?,
        BANK,
        "the HIRC Entry owns the ID"
    );
    assert_eq!(
        span(id_site, bank)?,
        wwise_short_id(EVENT).to_le_bytes().as_slice(),
        "exact HIRC ID word"
    );
    assert!(
        sites
            .iter()
            .any(|site| site["scope"] == "archiveDirectoryMetadata"
                && site.pointer("/location/field").and_then(Value::as_str) == Some("path")),
        "the joined result also credits the directory path"
    );
    Ok(())
}

fn assert_no_entity_uses(reply: &Value) -> TestResult {
    let uses = reply
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("physical uses")?;
    for site in uses {
        let source = proof::physical_source(reply, site)?;
        assert_ne!(
            source.pointer("/path/value").and_then(Value::as_str),
            Some(DATABASE),
            "declining the reverse lookup must not register an Entity source"
        );
    }
    Ok(())
}

#[test]
fn audio_search_retains_the_reverse_entity_selector_and_intrinsic_header() -> TestResult {
    let fixture = Fixture::new()?;
    let mut server = audio::serve(&fixture.install)?;
    let reply = server.ok(TOOL, &json!({"search": TRIGGER, "entities": 1}))?;
    let entities = reply
        .pointer("/triggers/0/entities")
        .ok_or("reverse Entity group")?;
    assert_eq!(
        entities.get("count"),
        Some(&json!(2)),
        "the count includes the unlisted matching Entity"
    );
    assert_eq!(
        entities.pointer("/listed/0/class_name"),
        Some(&json!(FIRST_ENTITY)),
        "the first physical Entity"
    );
    assert_eq!(
        entities
            .get("listed")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "only one Entity was requested"
    );

    let name_field = format!("{ROW}/entities/listed/0/class_name");
    proof::assert_marker(&reply, &name_field, true)?;
    let sites = proof::uses(&reply, &name_field)?;
    assert!(
        sites.iter().any(
            |site| site.pointer("/location/type").and_then(Value::as_str) == Some("dataForge")
                && site.pointer("/location/field").and_then(Value::as_str) == Some("recordName")
        ),
        "the listed class name credits its intrinsic Record header"
    );
    assert!(
        sites.iter().any(
            |site| site.pointer("/location/field").and_then(Value::as_str)
                == Some("pointerSelector")
        ),
        "the nested reference credits the actual DataForge pointer"
    );
    assert_body_selector(&reply, &sites, &fixture.database, FIRST_REFERENCE)?;
    let count_field = format!("{ROW}/entities/count");
    proof::assert_marker(&reply, &count_field, true)?;
    let count_sites = proof::uses(&reply, &count_field)?;
    assert_body_selector(&reply, &count_sites, &fixture.database, FIRST_REFERENCE)?;
    assert_body_selector(&reply, &count_sites, &fixture.database, SECOND_REFERENCE)?;
    proof::assert_marker(&reply, &format!("{ROW}/name"), false)
}

fn assert_body_selector(
    reply: &Value,
    sites: &[&Value],
    database: &[u8],
    authored: &str,
) -> TestResult {
    let selector = sites
        .iter()
        .find(|site| {
            (site["role"] == "selector" || site["role"] == "conversion")
                && site.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeDecodedBytes")
                && site.pointer("/location/field").and_then(Value::as_str) == Some("text")
                && span(site, database).is_ok_and(|bytes| bytes == authored.as_bytes())
        })
        .ok_or_else(|| format!("missing actual reverse lookup spelling {authored}"))?;
    assert_eq!(
        proof::source_path(reply, selector)?,
        DATABASE,
        "the reverse selector belongs to the DataForge Entry"
    );
    Ok(())
}

#[test]
fn audio_search_uses_page_relative_origins_for_the_next_definition() -> TestResult {
    let fixture = Fixture::new()?;
    let mut server = audio::serve(&fixture.install)?;
    let first = server.ok(TOOL, &json!({"search": "*", "limit": 1, "entities": 0}))?;
    let cursor = first["next_cursor"]
        .as_str()
        .ok_or("next definition cursor")?;
    assert_eq!(
        first.pointer("/triggers/0/name"),
        Some(&json!(TRIGGER)),
        "the first admitted definition"
    );
    proof::assert_marker(&first, &format!("{ROW}/name"), false)?;
    let second = server.ok(
        TOOL,
        &json!({"search": "*", "limit": 1, "entities": 0, "cursor": cursor}),
    )?;
    assert_eq!(
        second.pointer("/triggers/0/name"),
        Some(&json!(OTHER_TRIGGER)),
        "cursor advances to the next admitted definition"
    );
    assert_eq!(
        second.pointer("/triggers/0/definition"),
        Some(&json!(OTHER_ATL)),
        "the next original Entry"
    );
    assert!(
        second.get("next_cursor").is_none(),
        "two calls cover both definitions"
    );
    let field = format!("{ROW}/name");
    proof::assert_marker(&second, &field, true)?;
    let sites = proof::uses(&second, &field)?;
    let name_site = sites
        .into_iter()
        .find(|site| {
            site["role"] == "payload"
                && site.pointer("/location/field").and_then(Value::as_str) == Some("atl_name")
        })
        .ok_or("name inherited the second definition source")?;
    assert_eq!(
        proof::source_path(&second, name_site)?,
        OTHER_ATL,
        "the page-local address resolves the second physical definition"
    );
    assert_local_receipt(&second)
}

fn assert_local_receipt(reply: &Value) -> TestResult {
    let fields = reply
        .get("fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("receipt field paths")?;
    for address in fields.keys() {
        let relative = address
            .strip_prefix(ROOT)
            .ok_or("structured field address")?;
        assert!(
            reply.pointer(relative).is_some(),
            "{address} must resolve in this page"
        );
        for site in proof::uses(reply, address)? {
            proof::physical_source(reply, site)?;
        }
    }
    Ok(())
}

fn span<'a>(site: &Value, bytes: &'a [u8]) -> TestResult<&'a [u8]> {
    let index = |pointer: &str| -> TestResult<usize> {
        Ok(site
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or("source byte index")?
            .parse()?)
    };
    bytes
        .get(index("/location/indices/0")?..index("/location/indices/1")?)
        .ok_or_else(|| "original byte span is out of bounds".into())
}
