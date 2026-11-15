// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    TestAtl, TestBank, TestCodec, TestEntry, TestHircObject, TestPlaylistItem, TestWem,
    test_signal, wwise_short_id,
};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::support::{Install, Server, TestResult};

/// Enough playlist items for the object to exceed the 256 KiB response cap as JSON.
const PLAYLIST_ITEMS: usize = 3000;
const EVENTS: usize = 30;
const BANKS: usize = 12;

fn event_name(number: usize) -> String {
    format!("E_{number:02}")
}

fn many() -> TestResult<Install> {
    let names: Vec<String> = (0..EVENTS).map(event_name).collect();
    let events: Vec<&str> = names.iter().map(String::as_str).collect();
    let long = "P".repeat(300);
    let definitions = TestAtl::new("SC_Many")
        .trigger("Play_Many", &events)
        .trigger(&long, &["E_00"]);
    let mut entries = vec![TestEntry::zstd(
        "Data\\Libs\\GameAudio\\SC_Many.xml",
        definitions.to_cryxml(),
    )];
    for number in 0..BANKS {
        let bank = TestBank::new(u32::try_from(number + 1).unwrap_or(1))
            .object(TestHircObject::event(wwise_short_id("E_00"), &[1]))
            .bytes();
        entries.push(TestEntry::stored(
            format!("Data\\Sounds\\wwise\\Dup{number:02}.bnk"),
            bank,
        ));
    }
    Install::new(entries)
}

fn sounds() -> TestResult<Install> {
    Install::new(vec![
        TestEntry::stored(
            "Data\\Sounds\\wwise\\5100.wem",
            TestWem::pcm(TestCodec::Pcm, 1, 8000, &test_signal(8000 * 20, 1)).bytes(),
        ),
        TestEntry::stored(
            "Data\\Sounds\\wwise\\7100.wem",
            TestWem::encoded(TestCodec::Vorbis, 2, 48_000, 40_000_000).bytes(),
        ),
        TestEntry::stored(
            "Data\\Sounds\\wwise\\7200.wem",
            TestWem::pcm(TestCodec::Pcm, 1, 8000, &[]).bytes(),
        ),
        TestEntry::stored(
            "Data\\Sounds\\wwise\\7300.WEM",
            TestWem::pcm(TestCodec::Pcm, 1, 8000, &test_signal(8000, 1)).bytes(),
        ),
        TestEntry::zstd("Data\\Sounds\\wwise\\Huge.bnk", vec![0; 33 << 20]),
    ])
}

#[test]
fn events_are_complete_and_soundbank_previews_keep_their_totals() -> TestResult {
    let install = many()?;
    let mut server = serve(&install)?;
    let found = server.ok("search_audio_triggers", &json!({ "search": "Play_Many" }))?;
    let hit = &found["triggers"][0];
    assert_eq!(hit["events_total"], EVENTS, "all events are counted");
    assert_eq!(
        hit["events"].as_array().map(Vec::len),
        Some(EVENTS),
        "all original events listed"
    );
    assert_eq!(
        hit["events"][0]["soundbank_count"], BANKS,
        "all banks counted"
    );
    assert_eq!(
        hit["events"][0]["soundbanks"].as_array().map(Vec::len),
        Some(10),
        "10 listed"
    );
    let resolved = server.ok("resolve_audio_trigger", &json!({ "trigger": "Play_Many" }))?;
    assert_eq!(
        resolved["trigger"]["events_total"], EVENTS,
        "resolve counts"
    );
    assert_eq!(
        resolved["trigger"]["events"].as_array().map(Vec::len),
        Some(EVENTS),
        "resolve lists every original event"
    );
    let long = server.ok("search_audio_triggers", &json!({ "search": "PPPP" }))?;
    let name = "P".repeat(300);
    assert_eq!(
        long["triggers"][0]["name"].as_str(),
        Some(name.as_str()),
        "the complete original trigger name is retained"
    );
    Ok(())
}

const TRIGGER: &str = "Play_Limited";
const EVENT: &str = "E_Limited";

#[test]
fn an_unbounded_trigger_reaches_the_last_play_action_across_pages() -> TestResult {
    let install = wide_trigger()?;
    let mut server = serve(&install)?;
    let first = server.ok(
        "resolve_audio_trigger",
        &json!({ "trigger": TRIGGER, "limit": 2 }),
    )?;
    assert_eq!(
        first["truncated_by_limits"], false,
        "page size is not a traversal budget"
    );
    assert_eq!(
        first["counts"]["variants"], 3,
        "all three Play actions reached"
    );
    assert_eq!(
        first["breaks"],
        json!([]),
        "the wide Event has no broken links"
    );
    let cursor = first
        .get("next_cursor")
        .and_then(Value::as_str)
        .ok_or("third variant page")?;
    let last = server.ok(
        "resolve_audio_trigger",
        &json!({ "trigger": TRIGGER, "limit": 1, "cursor": cursor }),
    )?;
    assert_eq!(
        last["variants"][0]["wem_id"], 23,
        "the last authored Play action"
    );
    assert_eq!(
        last["truncated_by_limits"], false,
        "both pages describe the complete walk"
    );
    assert!(
        last.get("next_cursor").is_none(),
        "the third variant ends the listing"
    );
    Ok(())
}

#[test]
fn a_truncated_trigger_reports_the_traversal_cut_on_every_page() -> TestResult {
    let install = wide_trigger()?;
    let mut server = serve(&install)?;
    let first = server.ok(
        "resolve_audio_trigger",
        &json!({ "trigger": TRIGGER, "limit": 1, "max_visits": 5 }),
    )?;
    assert_eq!(
        first["truncated_by_limits"], true,
        "the Event has unfollowed references"
    );
    assert_eq!(
        first["counts"]["variants"], 2,
        "only two Play actions reached"
    );
    assert_eq!(
        first["variants"][0]["wem_id"], 21,
        "the first reached variant"
    );
    assert_eq!(
        first["breaks"][0]["kind"], "too_many_objects",
        "the traversal cutoff code"
    );
    assert!(
        first["breaks"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("visit budget of 5")),
        "the reason identifies the caller budget"
    );
    let cursor = first
        .get("next_cursor")
        .and_then(Value::as_str)
        .ok_or("second variant page")?;
    let second = server.ok(
        "resolve_audio_trigger",
        &json!({ "trigger": TRIGGER, "limit": 1, "max_visits": 5, "cursor": cursor }),
    )?;
    assert_eq!(
        second["variants"][0]["wem_id"], 22,
        "the second reached variant"
    );
    assert_eq!(
        second["truncated_by_limits"], true,
        "the cutoff is visible after the first page"
    );
    assert_eq!(
        second["breaks"],
        json!([]),
        "break details stay on the first page"
    );
    assert!(
        second.get("next_cursor").is_none(),
        "the reached variants end here"
    );
    assert_cursor_budget(&mut server, cursor)
}

fn assert_cursor_budget(server: &mut Server, cursor: &str) -> TestResult {
    for args in [
        json!({ "trigger": TRIGGER, "cursor": cursor, "max_visits": 6 }),
        json!({ "trigger": TRIGGER, "cursor": cursor }),
    ] {
        let error = server.error("resolve_audio_trigger", &args)?;
        assert!(
            error.contains("cursor"),
            "a different budget has a different listing: {error}"
        );
    }
    Ok(())
}

fn wide_trigger() -> TestResult<Install> {
    const STOP: u32 = 2;
    let mut actions = vec![1, 3];
    actions.resize(1_024, STOP);
    actions.push(4);
    let bank = TestBank::new(1)
        .object(TestHircObject::event(wwise_short_id(EVENT), &actions))
        .object(TestHircObject::action(1, 0x0403, 10))
        .object(TestHircObject::action(STOP, 0x0102, 10))
        .object(TestHircObject::action(3, 0x0403, 11))
        .object(TestHircObject::action(4, 0x0403, 12))
        .object(TestHircObject::sound(10, 21, 2, 0))
        .object(TestHircObject::sound(11, 22, 2, 0))
        .object(TestHircObject::sound(12, 23, 2, 0));
    trigger_install(&bank)
}

#[test]
fn a_late_visit_cutoff_keeps_the_last_break_preview_slot() -> TestResult {
    const MISSING: u32 = 30;
    const FIRST_MISSING: u32 = 100;
    let mut actions: Vec<_> = (FIRST_MISSING..FIRST_MISSING + MISSING).collect();
    actions.push(1);
    let bank = TestBank::new(1)
        .object(TestHircObject::event(wwise_short_id(EVENT), &actions))
        .object(TestHircObject::action(1, 0x0403, 10))
        .object(TestHircObject::sound(10, 21, 2, 0));
    let install = trigger_install(&bank)?;
    let mut server = serve(&install)?;
    let result = server.ok(
        "resolve_audio_trigger",
        &json!({ "trigger": TRIGGER, "max_visits": MISSING + 1 }),
    )?;
    assert_eq!(
        result["truncated_by_limits"], true,
        "the final Play action was cut off"
    );
    assert_eq!(
        result["counts"]["breaks"],
        MISSING + 1,
        "the total includes the unlisted breaks"
    );
    let breaks = result
        .get("breaks")
        .and_then(Value::as_array)
        .ok_or("break preview")?;
    let (cutoff, prefix) = breaks.split_last().ok_or("the reserved preview slot")?;
    assert_eq!(breaks.len(), 25, "the existing preview size is retained");
    assert_eq!(
        cutoff["kind"], "too_many_objects",
        "the cutoff remains visible"
    );
    assert!(
        prefix
            .iter()
            .map(|link| link.get("target").and_then(Value::as_u64))
            .eq((u64::from(FIRST_MISSING)..).take(24).map(Some)),
        "ordinary broken links keep their original order"
    );
    Ok(())
}

fn trigger_install(bank: &TestBank) -> TestResult<Install> {
    let definitions = TestAtl::new("SC_Limited").trigger(TRIGGER, &[EVENT]);
    Install::new(vec![
        TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Limited.xml",
            definitions.to_cryxml(),
        ),
        TestEntry::stored("Data\\Sounds\\wwise\\Limited.bnk", bank.bytes()),
    ])
}

#[test]
fn the_ten_second_clamp_alone_shortens_an_excerpt_and_says_so() -> TestResult {
    let install = sounds()?;
    let mut server = serve(&install)?;
    let result = server.call(
        "get_audio_excerpt",
        &json!({ "wem_id": 5100, "duration_seconds": 600.0 }),
    )?;
    let meta = &result["structuredContent"];
    assert_eq!(meta["duration_seconds"].as_f64(), Some(10.0), "10 seconds");
    assert_eq!(meta["frames"], 80_000, "frames");
    assert_eq!(meta["truncated"], true, "the clamp is reported");
    assert!(
        meta["bytes"].as_u64().is_some_and(|n| n < 1 << 20),
        "under the size cap"
    );
    let late = server.call(
        "get_audio_excerpt",
        &json!({ "wem_id": 5100, "start_seconds": 15.0, "duration_seconds": 10.0 }),
    )?;
    let meta = &late["structuredContent"];
    assert_eq!(
        meta["sound_seconds"].as_f64(),
        Some(20.0),
        "the whole sound"
    );
    assert_eq!(meta["duration_seconds"].as_f64(), Some(5.0), "to the end");
    assert_eq!(meta["truncated"], true, "the end of the sound");
    Ok(())
}

#[test]
fn an_excerpt_too_far_into_a_sound_or_of_an_empty_one_is_refused() -> TestResult {
    let install = sounds()?;
    let mut server = serve(&install)?;
    let far = server.error(
        "get_audio_excerpt",
        &json!({ "wem_id": 7100, "start_seconds": 600.0 }),
    )?;
    assert!(far.contains("too far in"), "{far}");
    let empty = server.error("get_audio_excerpt", &json!({ "wem_id": 7200 }))?;
    assert!(empty.contains("no audio frames"), "{empty}");
    let info = server.ok(
        "get_wem_info",
        &json!({ "path": "data/sounds/wwise/7300.wem" }),
    )?;
    assert_eq!(info["wem_id"], 7300, "the extension is matched in any case");
    Ok(())
}

#[test]
fn a_soundbank_beyond_the_size_cap_is_refused() -> TestResult {
    let install = sounds()?;
    let mut server = serve(&install)?;
    let message = server.error(
        "list_soundbank_wems",
        &json!({ "soundbank": "Data/Sounds/wwise/Huge.bnk" }),
    )?;
    assert!(message.contains("larger than"), "{message}");
    let message = server.error(
        "list_soundbank_objects",
        &json!({ "soundbank": "Data/Sounds/wwise/Huge.bnk" }),
    )?;
    assert!(message.contains("larger than"), "{message}");
    Ok(())
}

#[test]
fn an_object_larger_than_a_response_is_an_error_and_the_rest_of_the_bank_stays_listable()
-> TestResult {
    let items: Vec<TestPlaylistItem> = (0..PLAYLIST_ITEMS)
        .map(|_| TestPlaylistItem::new(10, 0))
        .collect();
    let bank = TestBank::new(1)
        .object(TestHircObject::music_playlist_items(7, &[10], &items))
        .object(TestHircObject::event(8, &[9]))
        .bytes();
    let install = Install::new(vec![TestEntry::stored(
        "Data\\Sounds\\wwise\\Big.bnk",
        bank,
    )])?;
    let mut server = serve(&install)?;
    let message = server.error(
        "list_soundbank_objects",
        &json!({ "soundbank": "Data/Sounds/wwise/Big.bnk" }),
    )?;
    assert!(
        message.contains("object 7") && message.contains("no filter narrows it further"),
        "{message}"
    );
    let rest = server.ok(
        "list_soundbank_objects",
        &json!({ "soundbank": "Data/Sounds/wwise/Big.bnk", "type": "event" }),
    )?;
    assert_eq!(rest["objects"][0]["id"], 8, "{rest}");
    Ok(())
}
