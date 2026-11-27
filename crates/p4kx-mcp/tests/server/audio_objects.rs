// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase, TestPlaylistItem,
};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::support::{Install, Server, TestResult};

const BANK: &str = "Data/Sounds/wwise/Music.bnk";
const OBJECT_ORDER: [u64; 8] = [10, 11, 12, 14, 15, 19, 16, 17];

fn install() -> TestResult<Install> {
    let bank = TestBank::new(0xD155_6D49)
        .object(TestHircObject::music_segment(10, &[11], &["Entry", "Exit"]))
        .object(TestHircObject::music_track(11, &[100, 101]))
        .object(TestHircObject::music_playlist_items(
            12,
            &[10],
            &[
                TestPlaylistItem::new(0, 2),
                TestPlaylistItem::new(10, 0).loops(3),
            ],
        ))
        .object(TestHircObject::container(
            TestContainerKind::Switch,
            14,
            &[15],
            TestNodeBase::Plain,
        ))
        .object(TestHircObject::sound(15, 100, 2, 4096))
        .object(TestHircObject::event(19, &[20]))
        .object(TestHircObject::raw(0xFE, 16, [0; 8]))
        .object(TestHircObject::raw(2, 17, [0; 3]))
        .bytes();
    let newer = TestBank::new(2)
        .version(160)
        .object(TestHircObject::event(1, &[2]))
        .object(TestHircObject::raw(7, 3, [0; 8]))
        .bytes();
    Install::new(vec![
        TestEntry::stored("Data\\Sounds\\wwise\\Music.bnk", bank),
        TestEntry::stored("Data\\Sounds\\wwise\\Newer.bnk", newer),
    ])
}

#[test]
fn objects_of_a_newer_bank_version_are_listed_as_unsupported() -> TestResult {
    let install = install()?;
    let mut server = serve(&install)?;
    let result = server.ok(
        "list_soundbank_objects",
        &json!({ "soundbank": "Data/Sounds/wwise/Newer.bnk" }),
    )?;
    assert_eq!(
        result["layouts"],
        json!({ "decoded": 0, "undecoded": 0, "unsupported": 2, "malformed": 0 }),
        "counted"
    );
    assert_eq!(result["objects"][0]["status"], "unsupported", "Event");
    assert_eq!(result["objects"][1]["type"], "actor-mixer");
    assert_eq!(result["objects"][1]["status"], "unsupported", "ActorMixer");
    Ok(())
}

fn ids(result: &Value) -> Vec<u64> {
    result["objects"]
        .as_array()
        .map_or_default(|objects| object_ids(objects))
}

#[test]
fn a_soundbank_lists_all_its_objects_in_bank_order_with_the_layout_counts() -> TestResult {
    let install = install()?;
    let mut server = serve(&install)?;
    let result = server.ok("list_soundbank_objects", &json!({ "soundbank": BANK }))?;
    assert_eq!(
        result["soundbank"], "Data\\Sounds\\wwise\\Music.bnk",
        "bank"
    );
    assert_eq!(result["bank_id"], 0xD155_6D49_u32, "bank ID");
    assert_eq!(
        (result["total"].as_u64(), result["matched"].as_u64()),
        (Some(8), Some(8))
    );
    assert_eq!(
        result["layouts"],
        json!({ "decoded": 6, "undecoded": 1, "unsupported": 0, "malformed": 1 }),
        "undecoded and malformed objects are counted"
    );
    let objects = collect_pages(&mut server, result, json!({ "soundbank": BANK }))?;
    assert_eq!(
        object_ids(&objects),
        OBJECT_ORDER,
        "bank order across byte-bounded pages"
    );
    let statuses: Vec<_> = objects
        .iter()
        .filter_map(|object| object["status"].as_str())
        .collect();
    assert_eq!(
        statuses.len(),
        OBJECT_ORDER.len(),
        "every original object has a status"
    );
    assert_eq!(
        statuses.get(6..),
        Some(["undecoded", "malformed"].as_slice()),
        "undecoded and malformed objects remain in bank order",
    );
    assert!(
        objects
            .last()
            .and_then(|object| object["error"].as_str())
            .is_some(),
        "the final malformed object has its error",
    );
    Ok(())
}

fn object_ids(objects: &[Value]) -> Vec<u64> {
    objects
        .iter()
        .filter_map(|object| object["id"].as_u64())
        .collect()
}

fn collect_pages(
    server: &mut Server,
    mut page: Value,
    mut arguments: Value,
) -> TestResult<Vec<Value>> {
    let header = |page: &Value| {
        [
            page["total"].clone(),
            page["matched"].clone(),
            page["layouts"].clone(),
        ]
    };
    let expected = header(&page);
    let matched = page
        .get("matched")
        .and_then(Value::as_u64)
        .ok_or("matched count")?;
    let matched = usize::try_from(matched)?;
    let mut objects: Vec<Value> = Vec::new();
    loop {
        assert_eq!(
            header(&page),
            expected,
            "continuation retains the bank and filter counters"
        );
        let batch = page
            .get_mut("objects")
            .and_then(Value::as_array_mut)
            .ok_or("objects")?;
        let progressed = !batch.is_empty();
        objects.append(batch);
        let cursor = match page.get("next_cursor") {
            Some(Value::String(cursor)) => cursor,
            None | Some(Value::Null) => {
                assert_eq!(objects.len(), matched, "complete original selection");
                return Ok(objects);
            }
            Some(_) => return Err("unexpected continuation cursor type".into()),
        };
        assert!(
            progressed && objects.len() < matched,
            "each continuation adds whole original objects"
        );
        arguments
            .as_object_mut()
            .ok_or("listing arguments")?
            .insert("cursor".to_owned(), json!(cursor));
        page = server.ok("list_soundbank_objects", &arguments)?;
    }
}

#[test]
fn the_objects_have_the_shape_of_p4kx_bnk_objects_json() -> TestResult {
    let install = install()?;
    let mut server = serve(&install)?;
    let playlist = server.ok(
        "list_soundbank_objects",
        &json!({ "soundbank": BANK, "id": 12 }),
    )?;
    let object = &playlist["objects"][0];
    assert_eq!(object["type"], "music-playlist", "type");
    assert_eq!(object["index"], 2, "position in the HIRC section");
    assert_eq!(object["children"], json!([10]), "children");
    assert_eq!(object["detail"]["items"][1]["loops"], 3, "playlist item");
    let segment = server.ok(
        "list_soundbank_objects",
        &json!({ "soundbank": BANK, "type": "music-segment" }),
    )?;
    assert_eq!(
        segment["objects"][0]["detail"]["markers"][1]["name"], "Exit",
        "segment marker"
    );
    Ok(())
}

#[test]
fn filters_narrow_the_listing_and_matched_counts_them() -> TestResult {
    let install = install()?;
    let mut server = serve(&install)?;
    let sounds = server.ok(
        "list_soundbank_objects",
        &json!({ "soundbank": BANK, "type": "sound" }),
    )?;
    assert_eq!(ids(&sounds), [15, 17], "the Sound and the malformed one");
    assert_eq!(sounds["matched"], 2, "matched");
    assert_eq!(sounds["total"], 8, "the whole bank");
    let none = server.ok(
        "list_soundbank_objects",
        &json!({ "soundbank": BANK, "type": "event", "id": 15 }),
    )?;
    assert_eq!(
        (none["matched"].as_u64(), ids(&none).len()),
        (Some(0), 0),
        "none"
    );
    let message = server.error(
        "list_soundbank_objects",
        &json!({ "soundbank": BANK, "type": "Sound" }),
    )?;
    assert!(
        message.contains("music-playlist"),
        "lists the types: {message}"
    );
    Ok(())
}

#[test]
fn pages_continue_with_the_cursor_and_the_limit_is_bounded() -> TestResult {
    let install = install()?;
    let mut server = serve(&install)?;
    let first = server.ok(
        "list_soundbank_objects",
        &json!({ "soundbank": BANK, "limit": 3 }),
    )?;
    assert_eq!(ids(&first), [10, 11, 12], "the first page");
    let cursor = first["next_cursor"].as_str().ok_or("a next page")?;
    let second = server.ok(
        "list_soundbank_objects",
        &json!({ "soundbank": BANK, "limit": 3, "cursor": cursor }),
    )?;
    assert_eq!(ids(&second), [14, 15, 19], "the second page");
    let third = server.ok(
        "list_soundbank_objects",
        &json!({
            "soundbank": BANK,
            "limit": 3,
            "cursor": second["next_cursor"].as_str().ok_or("a cursor")?
        }),
    )?;
    assert_eq!(ids(&third), [16, 17], "the last page");
    assert_eq!(third["next_cursor"], Value::Null, "no further page");
    let arguments = json!({ "soundbank": BANK, "limit": 4_000_000_000_u64 });
    let huge = server.ok("list_soundbank_objects", &arguments)?;
    let objects = collect_pages(&mut server, huge, arguments)?;
    assert_eq!(
        object_ids(&objects),
        OBJECT_ORDER,
        "a large requested limit still enumerates the full bank"
    );
    let other = server.error(
        "list_soundbank_objects",
        &json!({ "soundbank": BANK, "type": "sound", "cursor": cursor }),
    )?;
    assert!(
        other.contains("cursor"),
        "a cursor of another listing: {other}"
    );
    Ok(())
}

#[test]
fn an_unknown_soundbank_is_an_error() -> TestResult {
    let install = install()?;
    let mut server = serve(&install)?;
    let message = server.error(
        "list_soundbank_objects",
        &json!({ "soundbank": "nope.bnk" }),
    )?;
    assert!(message.contains("nope.bnk"), "{message}");
    Ok(())
}
