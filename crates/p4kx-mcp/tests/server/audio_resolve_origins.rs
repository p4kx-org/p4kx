// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_testkit::{
    TestAtl, TestBank, TestCodec, TestEntry, TestHircObject, TestWem, wwise_short_id,
};
use serde_json::{Value, json};

use crate::audio;
use crate::audio_origin_assertions as proof;
use crate::support::{Install, TestResult};

const TOOL: &str = "resolve_audio_trigger";
const ATL: &str = "Data/Libs/GameAudio/SC_RouteWitness.xml";
const TRIGGER: &str = "Synthetic_Route";
const EVENT: &str = "Synthetic_Trip";
const HOME: &str = "Data/Sounds/wwise/WitnessHome.bnk";
const FOREIGN: &str = "Data/Sounds/wwise/WitnessForeign.bnk";
const FOREIGN_NAME: &str = "WitnessForeign";
const PLAY: u16 = 0x0403;
const ACTION: u32 = 101;
const BROKEN_ACTION: u32 = 102;
const MISSING_ACTION: u32 = 103;
const SOUND: u32 = 201;
const MISSING_TARGET: u32 = 202;
const MEDIA: u32 = 301;
const DIDX_WORD: usize = 4;
const DIDX_RECORD: usize = 3 * DIDX_WORD;
const FIRST_MEDIA_RECORD: usize = 1;
const ROOT: &str = "/structuredContent";
const VARIANT: &str = "/structuredContent/variants/0";

struct Fixture {
    install: Install,
    home: Vec<u8>,
    foreign: Vec<u8>,
    first_len: usize,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let foreign_id = wwise_short_id(FOREIGN_NAME);
        let home = TestBank::new(wwise_short_id("WitnessHome"))
            .object(TestHircObject::event(
                wwise_short_id(EVENT),
                &[ACTION, MISSING_ACTION, BROKEN_ACTION],
            ))
            .object(TestHircObject::action_in_bank(
                ACTION, PLAY, SOUND, foreign_id,
            ))
            .object(TestHircObject::action_in_bank(
                BROKEN_ACTION,
                PLAY,
                MISSING_TARGET,
                foreign_id,
            ))
            .bytes();
        let chosen = TestWem::pcm(TestCodec::Pcm, 1, 8_000, &[3, 2, 1]).bytes();
        let first_len = chosen.len();
        let later = TestWem::pcm(TestCodec::Pcm, 1, 16_000, &[6, 5, 4, 3, 2, 1]).bytes();
        let foreign = TestBank::new(foreign_id)
            .wem(MEDIA - 1, b"seed".to_vec())
            .wem(MEDIA, chosen)
            .wem(MEDIA, later)
            .object(TestHircObject::sound(SOUND, MEDIA, 0, 0))
            .bytes();
        let definitions = TestAtl::new("RouteWitness")
            .trigger(TRIGGER, &[EVENT])
            .to_xml();
        let install = Install::new(vec![
            TestEntry::stored(ATL, definitions.into_bytes()),
            TestEntry::deflated(HOME, home.as_slice()).encrypted(),
            TestEntry::stored(FOREIGN, foreign.as_slice()),
        ])?;
        Ok(Self {
            install,
            home,
            foreign,
            first_len,
        })
    }
}

#[test]
fn resolve_audio_credits_visited_objects_and_first_physical_didx_record() -> TestResult {
    let fixture = Fixture::new()?;
    let mut server = audio::serve(&fixture.install)?;
    let reply = server.ok(TOOL, &json!({"trigger": TRIGGER}))?;
    assert_route(&reply, &fixture)?;
    assert_selected_media(&reply, &fixture)?;
    assert_receipt(&reply)?;
    let schema = server
        .tools()?
        .into_iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some(TOOL))
        .ok_or("audio resolve tool schema")?;
    jsonschema::validator_for(schema.get("outputSchema").ok_or("resolve output schema")?)?
        .validate(&reply)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn assert_route(reply: &Value, fixture: &Fixture) -> TestResult {
    assert_eq!(
        reply.pointer("/counts/variants").and_then(Value::as_u64),
        Some(1),
        "only the actual visited Sound yields a variant"
    );
    proof::assert_marker(reply, "/structuredContent/trigger/events/0/name", false)?;
    assert_eq!(
        reply.pointer("/variants/0/sound_id"),
        Some(&json!(SOUND)),
        "the foreign Sound supplies the variant"
    );
    assert_eq!(
        reply.pointer("/variants/0/wem_id"),
        Some(&json!(MEDIA)),
        "the selected Sound's embedded media ID"
    );
    let route = [
        (HOME, fixture.home.as_slice(), wwise_short_id(EVENT)),
        (HOME, fixture.home.as_slice(), ACTION),
        (FOREIGN, fixture.foreign.as_slice(), SOUND),
    ];
    for (index, (bank, bytes, id)) in route.into_iter().enumerate() {
        let field = format!("{VARIANT}/path/{index}/id");
        let used = use_at(reply, &field, "bnkHircObject", "id", bank)?;
        assert_eq!(
            decoded(used, bytes)?,
            id.to_le_bytes().as_slice(),
            "original HIRC ID at hop {index}"
        );
        assert_eq!(
            used.get("role").and_then(Value::as_str),
            Some("payload"),
            "hop {index} owns its emitted ID"
        );
    }
    let final_id = format!("{VARIANT}/sound_id");
    proof::assert_marker(reply, &final_id, true)?;
    for (namespace, label, expected) in [
        ("bnkHircEvent", "actionId", ACTION),
        ("bnkHircAction", "targetId", SOUND),
        ("bnkHircAction", "playBankId", wwise_short_id(FOREIGN_NAME)),
    ] {
        assert_selector(
            reply,
            &final_id,
            (namespace, label),
            &fixture.home,
            expected,
        )?;
    }
    Ok(())
}

fn assert_selected_media(reply: &Value, fixture: &Fixture) -> TestResult {
    let raw_didx = section(&fixture.foreign, *b"DIDX")?;
    let row_start = raw_didx.start + FIRST_MEDIA_RECORD * DIDX_RECORD;
    let row = fixture
        .foreign
        .get(row_start..row_start + DIDX_RECORD)
        .ok_or("first matched DIDX row")?;
    let load = |word| -> TestResult<u32> {
        let start = word * DIDX_WORD;
        let bytes = row.get(start..start + DIDX_WORD).ok_or("DIDX value")?;
        Ok(u32::from_le_bytes(bytes.try_into()?))
    };
    assert_eq!(
        load(0)?,
        MEDIA,
        "the earlier physical row owns the media ID"
    );
    let offset = usize::try_from(load(1)?)?;
    let first_len = usize::try_from(load(2)?)?;

    assert_eq!(
        first_len, fixture.first_len,
        "independent DIDX record selects the first payload"
    );
    let location = reply
        .pointer("/variants/0/location")
        .ok_or("embedded location")?;
    assert_eq!(
        location.get("path"),
        Some(&json!(FOREIGN)),
        "physically selected bank"
    );
    assert_eq!(
        location.get("offset"),
        Some(&json!(offset)),
        "DATA-relative offset from the first matched index row"
    );
    assert_eq!(
        location.get("len"),
        Some(&json!(first_len)),
        "the later duplicate has a different length"
    );
    for (field, label, word) in [("offset", "dataOffset", 1), ("len", "dataLength", 2)] {
        let address = format!("{VARIANT}/location/{field}");
        proof::assert_marker(reply, &address, true)?;
        let source = use_at(reply, &address, "bnkDidx", label, FOREIGN)?;
        let word_start = row_start + word * DIDX_WORD;
        assert_eq!(
            span(source)?,
            word_start..word_start + DIDX_WORD,
            "the receipt pins the earlier physical DIDX row"
        );
        assert_eq!(
            source.get("effectiveEncryption").and_then(Value::as_str),
            Some("unencrypted"),
            "the WEM metadata word physically comes from the clear foreign bank"
        );
    }
    Ok(())
}

#[test]
fn resolve_audio_breaks_retain_failed_route_selectors_without_wem_words() -> TestResult {
    let fixture = Fixture::new()?;
    let mut server = audio::serve(&fixture.install)?;
    let reply = server.ok(TOOL, &json!({"trigger": TRIGGER}))?;
    assert_eq!(
        reply.pointer("/counts/breaks"),
        Some(&json!(2)),
        "the two attempted branches each break"
    );
    for (index, target, namespace, label) in [
        (0, MISSING_ACTION, "bnkHircEvent", "actionId"),
        (1, MISSING_TARGET, "bnkHircAction", "targetId"),
    ] {
        let row = format!("/breaks/{index}");
        assert_eq!(
            reply.pointer(&format!("{row}/kind")),
            Some(&json!("missing")),
            "the resolver's actual outcome"
        );
        assert_eq!(
            reply.pointer(&format!("{row}/target")),
            Some(&json!(target)),
            "the missing target ID is retained"
        );
        let target_field = format!("{ROOT}{row}/target");
        let origin = proof::origin(&reply, &target_field)?;
        assert_eq!(
            origin.get("containsEncryptedSource"),
            Some(&json!(true)),
            "the original failed selector remains encrypted-derived"
        );
        assert_selector(
            &reply,
            &target_field,
            (namespace, label),
            &fixture.home,
            target,
        )?;
        assert_no_wem(&reply, &format!("{ROOT}{row}"))?;
    }
    Ok(())
}

fn assert_no_wem(reply: &Value, field: &str) -> TestResult {
    for member in ["", "/target", "/kind", "/message"] {
        for used in proof::uses(reply, &format!("{field}{member}"))? {
            let namespace = used
                .pointer("/location/namespace")
                .and_then(Value::as_str)
                .unwrap_or_default();
            assert!(
                namespace != "bnkDidx" && !namespace.starts_with("bnkWem"),
                "a route failure uses no embedded WEM fields"
            );
        }
    }
    Ok(())
}

fn assert_selector(
    reply: &Value,
    field: &str,
    (namespace, label): (&str, &str),
    original: &[u8],
    expected: u32,
) -> TestResult {
    let selector = use_at(reply, field, namespace, label, HOME)?;
    assert_eq!(
        selector.get("role").and_then(Value::as_str),
        Some("selector"),
        "the consumed route decision at {field}"
    );
    assert_eq!(
        decoded(selector, original)?,
        expected.to_le_bytes().as_slice(),
        "{field} retains the actual selector word"
    );
    Ok(())
}

fn use_at<'a>(
    reply: &'a Value,
    field: &str,
    namespace: &str,
    label: &str,
    owner: &str,
) -> TestResult<&'a Value> {
    let mut matched = proof::uses(reply, field)?.into_iter().filter(|used| {
        used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
            && used.pointer("/location/field").and_then(Value::as_str) == Some(label)
            && proof::source_path(reply, used).is_ok_and(|path| path == owner)
    });
    let used = matched
        .next()
        .ok_or_else(|| format!("missing original {namespace}/{label} at {field}"))?;
    assert!(
        matched.next().is_none(),
        "one actual {namespace}/{label} on this branch"
    );
    Ok(used)
}

fn span(site: &Value) -> TestResult<Range<usize>> {
    let index = |pointer| -> TestResult<usize> {
        Ok(site
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or("original byte index")?
            .parse()?)
    };
    Ok(index("/location/indices/0")?..index("/location/indices/1")?)
}

fn decoded<'a>(site: &Value, original: &'a [u8]) -> TestResult<&'a [u8]> {
    original
        .get(span(site)?)
        .ok_or_else(|| "invalid decoded-bank range".into())
}

fn section(bank: &[u8], name: [u8; 4]) -> TestResult<Range<usize>> {
    const SECTION_HEADER: usize = 8;
    let mut offset = 0;
    while let Some(header) = bank.get(offset..offset + SECTION_HEADER) {
        let length = header.get(4..).ok_or("bank section length word")?;
        let start = offset + SECTION_HEADER;
        let end = start + usize::try_from(u32::from_le_bytes(length.try_into()?))?;
        if header.get(..4) == Some(name.as_slice()) && bank.get(start..end).is_some() {
            return Ok(start..end);
        }
        offset = end;
    }
    Err("fixture bank section missing".into())
}

fn assert_receipt(reply: &Value) -> TestResult {
    let origins = reply
        .get("fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("resolve field origins")?;
    for address in origins.keys() {
        let relative = address.strip_prefix(ROOT).ok_or("rooted field origin")?;
        assert!(
            reply.pointer(relative).is_some(),
            "{address} names a field on the current page"
        );
        for usage in proof::uses(reply, address)? {
            proof::physical_source(reply, usage)?;
        }
    }
    Ok(())
}
