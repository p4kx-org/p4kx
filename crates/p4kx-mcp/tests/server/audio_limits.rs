// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    TestAtl, TestBank, TestCodec, TestEntry, TestHircObject, TestPlaylistItem, TestWem,
    test_signal, wwise_short_id,
};
use serde_json::json;

use crate::audio::serve;
use crate::support::{Install, TestResult};

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
fn events_and_soundbanks_per_trigger_are_capped_with_their_totals() -> TestResult {
    let install = many()?;
    let mut server = serve(&install)?;
    let found = server.ok("search_audio_triggers", &json!({ "search": "Play_Many" }))?;
    let hit = &found["triggers"][0];
    assert_eq!(hit["events_total"], EVENTS, "all events are counted");
    assert_eq!(
        hit["events"].as_array().map(Vec::len),
        Some(25),
        "25 listed"
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
        Some(25),
        "resolve lists 25"
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
