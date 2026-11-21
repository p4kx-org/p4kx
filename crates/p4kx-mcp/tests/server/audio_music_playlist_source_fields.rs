// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestEntry, TestHircObject};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::support::{Install, TestResult};

#[test]
fn music_playlist_stinger_source_fields_mcp_expose_schema_and_encrypted_words() -> TestResult {
    let path = "Data/Sounds/wwise/Playlist.bnk";
    let original = TestBank::new(21)
        .version(154)
        .object(TestHircObject::music_playlist(11, &[9]))
        .bytes();
    let install = Install::new(vec![TestEntry::zstd(path, original.clone()).encrypted()])?;
    let mut server = serve(&install)?;
    let report = server.ok(
        "list_soundbank_objects",
        &json!({"soundbank": path, "id": 11}),
    )?;
    let source = report
        .pointer("/objects/0/source_fields")
        .ok_or("MusicPlaylist source fields")?;
    assert_eq!(
        source.pointer("/layout/value"),
        Some(&json!("decoded")),
        "/layout/value"
    );
    assert_eq!(
        source.pointer("/stinger_count/value"),
        Some(&json!(1)),
        "/stinger_count/value"
    );
    let word = source
        .pointer("/stingers/0/dont_repeat_time/bits")
        .ok_or("original stinger repeat-time word")?;
    assert_eq!(word.get("value"), Some(&json!(0)), "value");
    assert_eq!(
        word.get("containsEncryptedSource"),
        Some(&json!(true)),
        "containsEncryptedSource"
    );
    assert_eq!(
        word.get("sourceSetComplete"),
        Some(&json!(true)),
        "sourceSetComplete"
    );
    check_original_word(source, word, &original)?;

    let schema = server
        .tools()?
        .into_iter()
        .find(|tool| tool.get("name") == Some(&json!("list_soundbank_objects")))
        .ok_or("soundbank objects tool")?;
    assert_eq!(
        schema.pointer("/outputSchema/$defs/MusicNodeObjectSourceFields/properties/stingers/type"),
        Some(&json!("array")),
        "discoverable typed stinger schema",
    );
    let validator = jsonschema::validator_for(schema.get("outputSchema").ok_or("output schema")?)?;
    validator
        .validate(&report)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn check_original_word(source: &Value, word: &Value, original: &[u8]) -> TestResult {
    let refs = word["sourceUsesRefs"]
        .as_array()
        .ok_or("word source references")?;
    let [reference] = refs.as_slice() else {
        return Err("the raw word needs one payload reference".into());
    };
    let uses = source
        .pointer("/source_inventory/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let receipt = uses
        .iter()
        .find(|row| row.get("id") == Some(reference))
        .ok_or("repeat-time source use")?;
    assert_eq!(
        receipt["role"], "payload",
        "the original word supplied the value"
    );
    assert_eq!(
        receipt.pointer("/location/field").and_then(Value::as_str),
        Some("musicStingerDontRepeatTime"),
        "original scheduling field",
    );
    let range = receipt
        .pointer("/location/indices")
        .and_then(Value::as_array)
        .ok_or("original word range")?;
    let [start, end] = range.as_slice() else {
        return Err("two half-open word offsets expected".into());
    };
    let start = start.as_str().ok_or("start offset")?.parse::<usize>()?;
    let end = end.as_str().ok_or("end offset")?.parse::<usize>()?;
    let prefix = [0x5100_0001_u32, 0x5100_0002, 0, 0].map(u32::to_le_bytes);
    let prefix = prefix.as_flattened();
    let stinger = original
        .windows(prefix.len())
        .position(|bytes| bytes == prefix)
        .ok_or("authored trigger, segment, sync and cue words")?;
    let repeat_word = stinger + prefix.len();
    assert_eq!(
        start..end,
        repeat_word..repeat_word + size_of::<u32>(),
        "the receipt addresses the independent stinger position",
    );
    assert_eq!(
        original.get(start..end),
        Some(0_u32.to_le_bytes().as_slice()),
        "the receipt covers the original four bytes",
    );
    Ok(())
}
