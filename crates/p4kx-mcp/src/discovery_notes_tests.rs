// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use rmcp::model::CallToolResult;
use serde_json::{Value, json};

use super::{DiscoveryNotes, NOTICE_BYTES};
use crate::noted::Notes;

#[test]
fn repeated_messages_are_deduplicated_and_taking_notes_clears_the_call() {
    let mut pending = DiscoveryNotes::default();
    pending.note("one failure".to_owned());
    pending.note("one failure".to_owned());
    assert_eq!(
        pending.take(),
        ["one failure"],
        "one source diagnostic is not repeated"
    );
    assert_eq!(
        pending.take(),
        Vec::<String>::new(),
        "notes do not leak into a later call"
    );
}

#[test]
fn oversize_diagnostics_are_reported_as_omitted_occurrences() {
    let mut pending = DiscoveryNotes::default();
    pending.note("x".repeat(NOTICE_BYTES));
    pending.note("second".to_owned());
    let messages = pending.take();
    assert_eq!(
        messages.len(),
        2,
        "the retained detail and explicit omission report both remain"
    );
    assert_eq!(
        messages.first().map(String::len),
        Some(NOTICE_BYTES),
        "the named notice cap is exact"
    );
    assert!(
        messages
            .last()
            .is_some_and(|text| text.contains("notices omitted: 1")),
        "omission cannot masquerade as a complete diagnostic list"
    );
}

#[test]
fn discovery_notices_keep_the_structured_payload_and_key_metadata_contract()
-> Result<(), Box<dyn Error>> {
    let payload = json!({"channels": ["LIVE"], "total": 1});
    let mut result = CallToolResult::structured(payload.clone());
    let notes = Notes {
        archive: None,
        key: None,
        discovery: vec!["Launcher discovery incomplete: synthetic detail".to_owned()],
    };
    notes.apply(&mut result);
    let response = serde_json::to_value(result)?;
    assert_eq!(
        response.get("structuredContent"),
        Some(&payload),
        "discovery adds notices without replacing the typed result"
    );
    assert!(
        response.get("_meta").is_none(),
        "discovery does not invent key metadata"
    );
    let content = response
        .get("content")
        .and_then(Value::as_array)
        .ok_or("content blocks")?;
    assert!(
        content
            .iter()
            .any(|block| block.get("text").and_then(Value::as_str)
                == Some("Launcher discovery incomplete: synthetic detail")),
        "the measured diagnostic is visible to the MCP client"
    );
    Ok(())
}
