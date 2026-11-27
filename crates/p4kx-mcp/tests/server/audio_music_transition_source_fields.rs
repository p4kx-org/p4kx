// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestEntry, TestHircObject};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::support::{Install, TestResult};

const PLAYLIST: &str = "Data/Sounds/wwise/TestPlaylist.bnk";
const SWITCH: &str = "Data/Sounds/wwise/TestSwitch.bnk";
const OWNER: u32 = 11;
const CHILD: u32 = 13;
const SOURCE_ID: u32 = 0x5200_0001;
const STINGER: [u32; 6] = [0x5100_0001, 0x5100_0002, 0, 0, 0, 0];

#[test]
fn music_transition_mcp_validates_both_encrypted_owners_and_the_original_id_word() -> TestResult {
    let playlist = bank(TestHircObject::music_playlist(OWNER, &[CHILD]));
    let switch = bank(TestHircObject::music_switch(OWNER, &[CHILD], 17, &[CHILD]));
    let install = Install::new(vec![
        TestEntry::zstd(PLAYLIST, playlist.clone()).encrypted(),
        TestEntry::zstd(SWITCH, switch.clone()).encrypted(),
    ])?;
    let mut server = serve(&install)?;
    let tool = server
        .tools()?
        .into_iter()
        .find(|tool| tool.get("name") == Some(&json!("list_soundbank_objects")))
        .ok_or("soundbank objects tool")?;
    let schema = tool
        .get("outputSchema")
        .ok_or("soundbank objects output schema")?;
    assert!(
        schema
            .pointer("/$defs/MusicNodeObjectSourceFields/properties/transition_rules")
            .is_some(),
        "the tool advertises the common transition table",
    );
    let validator = jsonschema::validator_for(schema)?;
    for (path, original) in [(PLAYLIST, playlist.as_slice()), (SWITCH, switch.as_slice())] {
        let report = server.ok(
            "list_soundbank_objects",
            &json!({"soundbank": path, "id": OWNER}),
        )?;
        check_table(&report, original)?;
        validator
            .validate(&report)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn bank(owner: TestHircObject) -> Vec<u8> {
    TestBank::new(21).version(154).object(owner).bytes()
}

fn check_table(document: &Value, original: &[u8]) -> TestResult {
    let source = document
        .pointer("/objects/0/source_fields")
        .ok_or("original source fields")?;
    let table = source
        .get("transition_rules")
        .ok_or("original transition rules")?;
    assert_eq!(
        table.pointer("/complete_rules/value"),
        Some(&json!(2)),
        "both original rules"
    );
    assert_eq!(
        table.pointer("/rows/1/side/value"),
        Some(&json!("source")),
        "first authored source ID"
    );
    let word = table
        .pointer("/rows/1/id")
        .ok_or("first original source ID word")?;
    assert_eq!(
        word.get("value"),
        Some(&json!(SOURCE_ID)),
        "original nonzero word"
    );
    assert_eq!(
        word.get("containsEncryptedSource"),
        Some(&json!(true)),
        "owning encrypted archive entry"
    );
    assert_eq!(
        word.get("sourceSetComplete"),
        Some(&json!(true)),
        "complete physical evidence"
    );

    check_id_span(original, word_span(source, word)?)
}

fn word_span(
    source: &Value,
    word: &Value,
) -> Result<std::ops::Range<usize>, Box<dyn std::error::Error>> {
    let refs = word
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source refs")?;
    let [reference] = refs.as_slice() else {
        return Err("one original ID word receipt expected".into());
    };
    let inventory = source
        .pointer("/source_inventory/uses")
        .and_then(Value::as_array)
        .ok_or("source inventory")?;
    let receipt = inventory
        .iter()
        .find(|row| row.get("id") == Some(reference))
        .ok_or("original ID use")?;
    assert_eq!(
        receipt.get("role"),
        Some(&json!("payload")),
        "the original word supplies the value"
    );
    let offsets = receipt
        .pointer("/location/indices")
        .and_then(Value::as_array)
        .ok_or("word extent")?;
    let [start, end] = offsets.as_slice() else {
        return Err("two bank offsets expected".into());
    };
    let start = start.as_str().ok_or("start offset")?.parse::<usize>()?;
    let end = end.as_str().ok_or("end offset")?.parse::<usize>()?;
    Ok(start..end)
}

fn check_id_span(original: &[u8], actual: std::ops::Range<usize>) -> TestResult {
    const WORD: usize = size_of::<u32>();
    let prefix = STINGER.map(u32::to_le_bytes);
    let prefix = prefix.as_flattened();
    let stinger = original
        .windows(prefix.len())
        .position(|bytes| bytes == prefix)
        .ok_or("original stinger")?;
    let table_start = stinger + prefix.len();
    let expected = table_start + 2 * WORD; // Count of rules, then count of source IDs.
    assert_eq!(
        actual,
        expected..expected + WORD,
        "physical offset of rule zero's first source ID"
    );
    assert_eq!(
        original.get(actual),
        Some(SOURCE_ID.to_le_bytes().as_slice()),
        "actual BNK bytes"
    );
    Ok(())
}
