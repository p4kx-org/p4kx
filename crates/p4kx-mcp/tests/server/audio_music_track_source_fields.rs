// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestEntry, TestHircObject};
use serde_json::json;

use crate::audio::serve;
use crate::support::{Install, TestResult};

#[test]
fn music_track_source_fields_mcp_retains_encrypted_clip_and_fade_words() -> TestResult {
    let path = "Data/Sounds/wwise/Track.bnk";
    let original = TestBank::new(21)
        .version(154)
        .object(TestHircObject::music_switch_track(11, &[4], 19, &[3]))
        .bytes();
    let install = Install::new(vec![TestEntry::zstd(path, original).encrypted()])?;
    let mut server = serve(&install)?;
    let result = server.ok(
        "list_soundbank_objects",
        &json!({"soundbank": path, "id": 11}),
    )?;
    let source = result
        .pointer("/objects/0/source_fields")
        .ok_or("original MusicTrack fields")?;
    let track = source
        .pointer("/rows/1/track_id")
        .ok_or("original clip track ID")?;
    assert_eq!(track.get("value"), Some(&json!(1)), "authored track ID");
    assert_eq!(
        track.get("containsEncryptedSource"),
        Some(&json!(true)),
        "direct encrypted field"
    );
    let fade = source
        .pointer("/fields/switch/transition/source_fade/time/bits")
        .ok_or("source fade raw word")?;
    assert_eq!(
        fade.get("value"),
        Some(&json!(0)),
        "explicit zero source fade"
    );
    assert_eq!(
        fade.get("containsEncryptedSource"),
        Some(&json!(true)),
        "owning encrypted BNK"
    );
    let schema = server
        .tools()?
        .into_iter()
        .find(|tool| tool.get("name") == Some(&json!("list_soundbank_objects")))
        .ok_or("soundbank objects tool")?;
    assert_eq!(
        schema.pointer("/outputSchema/$defs/MusicTrackObjectSourceFields/properties/rows/type"),
        Some(&json!("array")),
        "discoverable typed schema"
    );
    Ok(())
}
