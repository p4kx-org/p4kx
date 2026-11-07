// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestAtl, TestEntry, other_test_key, test_key};
use serde_json::{Value, json};

use crate::support::{Install, Server, TestResult};

/// An Archive whose only ATL definition is encrypted, so the audio index of a call without the
/// key lacks it without the call failing.
pub(crate) fn install() -> TestResult<Install> {
    let definitions = TestAtl::new("SC_Ship").trigger("Play_Engine", &["Play_Engine_Start"]);
    Install::new(vec![
        TestEntry::stored("Data/plain.xml", b"<plain/>".to_vec()),
        TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Ship.xml",
            definitions.to_cryxml(),
        )
        .encrypted(),
    ])
}

pub(crate) fn search(server: &mut Server) -> TestResult<Value> {
    server.call("search_audio_triggers", &json!({ "search": "engine" }))
}

pub(crate) fn texts(result: &Value) -> Vec<&str> {
    result["content"]
        .as_array()
        .map(|blocks| blocks.iter().filter_map(|b| b["text"].as_str()).collect())
        .unwrap_or_default()
}

pub(crate) fn hint_of<'a>(result: &'a Value, needle: &str) -> Option<&'a str> {
    texts(result).into_iter().find(|text| text.contains(needle))
}

#[test]
fn a_call_that_left_encrypted_entries_unread_without_a_key_says_so() -> TestResult {
    let install = install()?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;

    let result = search(&mut server)?;

    assert_ne!(result["isError"], true, "the call does not fail: {result}");
    assert_eq!(result["_meta"]["archive_key"], "missing", "{result}");
    assert_eq!(result["_meta"]["encrypted_entries_unread"], 1, "{result}");
    let hint = hint_of(&result, "1 encrypted Entry")
        .ok_or_else(|| format!("a text block names the Entry: {result}"))?;
    assert!(hint.contains("P4KX_ARCHIVE_KEY"), "{hint}");
    Ok(())
}

#[test]
fn a_wrong_key_is_reported_as_wrong() -> TestResult {
    let install = install()?;
    let wrong = other_test_key().to_hex();
    let mut server = install.serve_without_key(&["--archive", "LIVE", "--key", &wrong])?;

    let result = search(&mut server)?;

    assert_eq!(result["_meta"]["archive_key"], "wrong", "{result}");
    assert_eq!(result["_meta"]["encrypted_entries_unread"], 1, "{result}");
    let hint = hint_of(&result, "does not fit")
        .ok_or_else(|| format!("a text block names the key: {result}"))?;
    assert!(!hint.contains(&wrong), "the key is not repeated: {hint}");
    Ok(())
}

#[test]
fn with_the_key_the_result_says_given_and_has_no_hint() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let result = search(&mut server)?;

    assert_eq!(result["_meta"]["archive_key"], "given", "{result}");
    assert_eq!(result["_meta"]["encrypted_entries_unread"], 0, "{result}");
    assert_eq!(texts(&result).len(), 1, "only the JSON text: {result}");
    Ok(())
}

#[test]
fn a_call_that_asks_for_no_encrypted_entry_without_a_key_adds_nothing() -> TestResult {
    let install = install()?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;

    let result = server.call("archive_info", &json!({}))?;

    assert!(result.get("_meta").is_none(), "{result}");
    assert_eq!(texts(&result).len(), 1, "only the JSON text: {result}");
    Ok(())
}

#[test]
fn a_later_call_does_not_repeat_the_demand_of_an_earlier_one() -> TestResult {
    let install = install()?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    let first = search(&mut server)?;
    assert_eq!(first["_meta"]["encrypted_entries_unread"], 1, "{first}");

    let later = server.call("archive_info", &json!({}))?;

    assert!(later.get("_meta").is_none(), "{later}");
    assert!(hint_of(&later, "encrypted Entr").is_none(), "{later}");
    Ok(())
}

#[test]
fn a_key_stored_between_calls_ends_the_note() -> TestResult {
    let install = install()?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    let before = server.call("archive_info", &json!({}))?;
    assert!(before.get("_meta").is_none(), "{before}");

    std::fs::write(
        install.config().join("archive.key"),
        format!("{}\n", test_key().to_hex()),
    )?;
    let after = search(&mut server)?;

    assert_eq!(after["_meta"]["archive_key"], "given", "{after}");
    assert_eq!(after["_meta"]["encrypted_entries_unread"], 0, "{after}");
    Ok(())
}

#[test]
fn a_call_that_fails_with_a_key_error_keeps_its_error_and_gets_no_second_hint() -> TestResult {
    let install = install()?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;

    let result = server.call(
        "read_entry_text",
        &json!({ "path": "Data/Libs/GameAudio/SC_Ship.xml" }),
    )?;

    assert_eq!(result["isError"], true, "{result}");
    assert!(result.get("_meta").is_none(), "{result}");
    assert_eq!(texts(&result).len(), 1, "one text, the error: {result}");
    Ok(())
}
