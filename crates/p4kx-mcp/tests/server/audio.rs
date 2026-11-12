// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestAtl, TestBank, TestCodec, TestContainerKind, TestEntity, TestEntry,
    TestHircObject, TestNodeBase, TestWem, audio_schema, entity_schema, test_signal,
    wwise_short_id,
};
use serde_json::{Value, json};

use crate::support::{Install, Server, TestResult};

const PLAY: u16 = 0x0403;

pub(crate) fn stereo(rate: u32, seconds: u32) -> Vec<u8> {
    TestWem::pcm(TestCodec::Pcm, 2, rate, &test_signal(rate * seconds, 2)).bytes()
}

fn ship_bank() -> Vec<u8> {
    let streamed = TestWem::pcm(TestCodec::Pcm, 1, 8000, &test_signal(8000, 1)).bytes();
    let prefetch = streamed.iter().copied().take(40).collect::<Vec<u8>>();
    TestBank::new(wwise_short_id("Ship"))
        .wem(5000, stereo(8000, 3))
        .wem(
            5001,
            TestWem::ima_adpcm(1, 8000, &test_signal(8000, 1)).bytes(),
        )
        .wem(5003, prefetch)
        .wem(6000, stereo(32_000, 12))
        .object(TestHircObject::event(
            wwise_short_id("Play_Engine_Start"),
            &[1],
        ))
        .object(TestHircObject::action(1, PLAY, 100))
        .object(TestTree::random(100, &[200, 201]))
        .object(TestHircObject::sound(200, 5000, 0, 0))
        .object(TestHircObject::sound(201, 5001, 0, 0))
        .object(TestHircObject::event(
            wwise_short_id("Play_Engine_Loop"),
            &[2],
        ))
        .object(TestHircObject::action(2, PLAY, 203))
        .object(TestHircObject::sound(203, 5003, 1, 0))
        .object(TestHircObject::event(
            wwise_short_id("Play_Shield_Up"),
            &[3],
        ))
        .object(TestHircObject::action(3, PLAY, 210))
        .object(TestHircObject::sound(210, 6000, 0, 0))
        .bytes()
}

struct TestTree;
impl TestTree {
    fn random(id: u32, children: &[u32]) -> TestHircObject {
        TestHircObject::container(TestContainerKind::Random, id, children, TestNodeBase::Full)
    }
}

pub(crate) fn install() -> TestResult<Install> {
    let ship = TestEntity::new(
        0x0079_c5d5_1678_4f8c_85ba_18ca_8f64_2af6,
        "TEST_Ship",
        "libs/foundry/records/entities/spaceships/test_ship.xml",
    )
    .audio_trigger("Play_Engine")
    .audio_trigger("Play_Undefined")
    .sound_effect("Shield Power Up", "Play_Shield")
    .audio_trigger("Play_Nowhere");
    let database = audio_schema(entity_schema(DataForgeBuilder::new())).record(ship.record());
    let definitions = TestAtl::new("SC_Ship")
        .trigger("Play_Engine", &["Play_Engine_Start", "Play_Engine_Loop"])
        .trigger("Play_Shield", &["Play_Shield_Up"])
        .trigger("Play_Nowhere", &["Play_In_No_Bank"])
        .bank("Ship.bnk");
    Install::new(vec![
        TestEntry::zstd("Data\\Game2.dcb", database.build()),
        TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Ship.xml",
            definitions.to_cryxml(),
        ),
        TestEntry::stored("Data\\Sounds\\wwise\\Ship.bnk", ship_bank()),
        TestEntry::stored(
            "Data\\Sounds\\wwise\\5003.wem",
            TestWem::pcm(TestCodec::Pcm, 1, 8000, &test_signal(8000, 1)).bytes(),
        ),
    ])
}

pub(crate) fn serve(install: &Install) -> TestResult<Server> {
    install.serve(&["--archive", "LIVE"])
}

fn names(value: &Value, key: &str) -> Vec<String> {
    value.as_array().map_or_default(|items| {
        items
            .iter()
            .filter_map(|item| item[key].as_str().map(str::to_owned))
            .collect()
    })
}

#[test]
fn a_search_lists_triggers_with_events_soundbanks_and_entities() -> TestResult {
    let install = install()?;
    let mut server = serve(&install)?;
    let plain = server.ok("search_audio_triggers", &json!({ "search": "engine" }))?;
    assert_eq!(
        plain["triggers"][0]["entities"],
        Value::Null,
        "the Entities are only looked up when asked for"
    );
    let result = server.ok(
        "search_audio_triggers",
        &json!({ "search": "engine", "entities": 3 }),
    )?;
    assert_eq!(result["total"], 1, "one trigger matches");
    assert_eq!(result["trigger_count"], 3, "three triggers are defined");
    let hit = &result["triggers"][0];
    assert_eq!(hit["name"], "Play_Engine", "the trigger");
    assert_eq!(hit["matched_trigger"], true, "the name matched");
    assert_eq!(
        names(&hit["events"], "name"),
        ["Play_Engine_Start", "Play_Engine_Loop"],
        "the events"
    );
    assert_eq!(
        hit["events"][0]["soundbanks"],
        json!([{ "path": "Data\\Sounds\\wwise\\Ship.bnk", "language": null }]),
        "the soundbank holding the event"
    );
    assert_eq!(hit["events"][0]["soundbank_count"], 1, "its count");
    assert_eq!(hit["events_total"], 2, "the events of the trigger");
    assert_eq!(hit["unmatched"], Value::Null, "it leads to a soundbank");
    assert_eq!(hit["entities"]["count"], 1, "one Entity names it");
    assert_eq!(
        hit["entities"]["listed"][0]["class_name"], "TEST_Ship",
        "the Entity"
    );
    Ok(())
}

#[test]
fn a_search_by_event_name_or_glob_finds_the_trigger_and_says_why_one_is_unmatched() -> TestResult {
    let install = install()?;
    let mut server = serve(&install)?;
    let by_event = server.ok(
        "search_audio_triggers",
        &json!({ "search": "shield_up", "entities": 0 }),
    )?;
    assert_eq!(
        names(&by_event["triggers"], "name"),
        ["Play_Shield"],
        "by event"
    );
    assert_eq!(
        by_event["triggers"][0]["matched_trigger"], false,
        "only the event matched"
    );
    assert_eq!(
        by_event["triggers"][0]["entities"],
        Value::Null,
        "entities: 0 skips the lookup"
    );
    let glob = server.ok(
        "search_audio_triggers",
        &json!({ "search": "Play_*o*", "entities": 0 }),
    )?;
    let listed = names(&glob["triggers"], "name");
    assert!(listed.contains(&"Play_Nowhere".to_owned()), "{listed:?}");
    let nowhere = glob["triggers"]
        .as_array()
        .and_then(|all| all.iter().find(|hit| hit["name"] == "Play_Nowhere"))
        .ok_or("Play_Nowhere")?;
    assert_eq!(nowhere["unmatched"]["kind"], "no_soundbank", "why");
    Ok(())
}

#[test]
fn a_search_pages_with_a_cursor_that_only_continues_its_own_search() -> TestResult {
    let install = install()?;
    let mut server = serve(&install)?;
    let first = server.ok(
        "search_audio_triggers",
        &json!({ "limit": 2, "entities": 0 }),
    )?;
    assert_eq!(
        (
            first["total"].as_u64(),
            first["triggers"].as_array().map(Vec::len)
        ),
        (Some(3), Some(2))
    );
    let cursor = first["next_cursor"].as_str().ok_or("a next page")?;
    let second = server.ok(
        "search_audio_triggers",
        &json!({ "limit": 2, "entities": 0, "cursor": cursor }),
    )?;
    assert_eq!(
        second["triggers"].as_array().map(Vec::len),
        Some(1),
        "the rest"
    );
    assert!(second.get("next_cursor").is_none(), "the last page");
    let message = server.error(
        "search_audio_triggers",
        &json!({ "search": "x", "cursor": cursor }),
    )?;
    assert!(
        message.contains("cursor"),
        "another search rejects it: {message}"
    );
    Ok(())
}

#[test]
fn an_entity_shows_its_trigger_event_soundbank_chain() -> TestResult {
    let install = install()?;
    let mut server = serve(&install)?;
    let result = server.ok("get_entity_sounds", &json!({ "entity": "test_ship" }))?;
    assert_eq!(result["entity"]["class_name"], "TEST_Ship", "the Entity");
    assert_eq!(
        (result["total"].as_u64(), result["matched"].as_u64()),
        (Some(4), Some(2))
    );
    let triggers = result["triggers"].as_array().ok_or("triggers")?;
    let by_name = |name: &str| triggers.iter().find(|trigger| trigger["name"] == name);
    let engine = by_name("Play_Engine").ok_or("Play_Engine")?;
    assert_eq!(
        names(&engine["events"], "name"),
        ["Play_Engine_Start", "Play_Engine_Loop"],
        "events"
    );
    assert_eq!(
        engine["events"][1]["soundbanks"][0]["path"], "Data\\Sounds\\wwise\\Ship.bnk",
        "soundbank"
    );
    assert_eq!(
        by_name("Play_Nowhere").ok_or("Play_Nowhere")?["unmatched"]["kind"],
        "no_soundbank"
    );
    assert_eq!(
        by_name("Play_Undefined").ok_or("Play_Undefined")?["unmatched"]["kind"],
        "no_definition"
    );
    let by_guid = server.ok(
        "get_entity_sounds",
        &json!({ "entity": "0079c5d5-1678-4f8c-85ba-18ca8f642af6", "limit": 1 }),
    )?;
    assert_eq!(
        by_guid["triggers"].as_array().map(Vec::len),
        Some(1),
        "paged"
    );
    assert!(by_guid["next_cursor"].is_string(), "more triggers follow");
    let message = server.error("get_entity_sounds", &json!({ "entity": "No_Such_Ship" }))?;
    assert!(message.contains("cannot find the Entity"), "{message}");
    Ok(())
}

#[test]
fn a_trigger_resolves_to_sound_variants_with_wem_ids_and_locations() -> TestResult {
    let install = install()?;
    let mut server = serve(&install)?;
    let result = server.ok(
        "resolve_audio_trigger",
        &json!({ "trigger": "play_engine" }),
    )?;
    assert_eq!(result["trigger"]["name"], "Play_Engine", "the trigger");
    assert_eq!(
        result["counts"],
        json!({ "variants": 3, "located": 3, "unlocated": 0, "plugins": 0, "breaks": 0 }),
        "counts"
    );
    let variants = result["variants"].as_array().ok_or("variants")?;
    let wems: Vec<u64> = variants
        .iter()
        .filter_map(|v| v["wem_id"].as_u64())
        .collect();
    assert_eq!(wems, [5000, 5001, 5003], "WEM IDs");
    assert_eq!(variants[0]["location"]["kind"], "soundbank", "embedded");
    assert_eq!(variants[2]["source"], "prefetch_streamed", "streamed");
    assert_eq!(
        variants[2]["location"],
        json!({ "kind": "entry", "path": "Data\\Sounds\\wwise\\5003.wem", "language": null, "len": variants[2]["location"]["len"] }),
        "the loose WEM Entry"
    );
    assert_eq!(
        variants[0]["path"][0]["kind"], "event",
        "path starts at the event"
    );
    let first = server.ok(
        "resolve_audio_trigger",
        &json!({ "trigger": "Play_Engine", "limit": 2 }),
    )?;
    assert_eq!(first["variants"].as_array().map(Vec::len), Some(2), "paged");
    let cursor = first["next_cursor"].as_str().ok_or("a next page")?;
    let second = server.ok(
        "resolve_audio_trigger",
        &json!({ "trigger": "Play_Engine", "limit": 2, "cursor": cursor }),
    )?;
    assert_eq!(
        second["variants"].as_array().map(Vec::len),
        Some(1),
        "the rest"
    );
    Ok(())
}

#[test]
fn an_unmatched_event_is_a_break_and_unknown_triggers_and_languages_are_errors() -> TestResult {
    let install = install()?;
    let mut server = serve(&install)?;
    let nowhere = server.ok(
        "resolve_audio_trigger",
        &json!({ "trigger": "Play_Nowhere" }),
    )?;
    assert_eq!(nowhere["counts"]["breaks"], 1, "one break");
    assert_eq!(nowhere["breaks"][0]["kind"], "no_soundbank", "its reason");
    let message = server.error(
        "resolve_audio_trigger",
        &json!({ "trigger": "Play_Undefined" }),
    )?;
    assert!(message.contains("search_audio_triggers"), "{message}");
    let message = server.error(
        "resolve_audio_trigger",
        &json!({ "trigger": "Play_Engine", "lang": "Klingon" }),
    )?;
    assert!(message.contains("no audio language"), "{message}");
    Ok(())
}
