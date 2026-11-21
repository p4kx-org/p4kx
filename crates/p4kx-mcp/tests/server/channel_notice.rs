// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::support::{Install, TestResult};

fn texts(result: &Value) -> Vec<&str> {
    result["content"]
        .as_array()
        .map_or_default(|blocks| blocks.iter().filter_map(|b| b["text"].as_str()).collect())
}

#[test]
fn a_test_channel_adds_a_note_to_the_response_and_keeps_the_structured_content() -> TestResult {
    let install = Install::standard()?;
    for channel in ["PTU", "EPTU", "HOTFIX", "TECH-PREVIEW"] {
        install.add_channel(channel)?;
        let mut server = install.serve(&[])?;

        let result = server.call("archive_info", &json!({ "archive": channel }))?;

        assert_ne!(result.get("isError"), Some(&Value::Bool(true)), "{result}");
        assert_eq!(result["structuredContent"]["channel"], channel);
        assert!(
            result["structuredContent"].get("notice").is_none(),
            "the note is not part of the data"
        );
        let notes: Vec<&str> = texts(&result)
            .into_iter()
            .filter(|text| text.contains("confidential"))
            .collect();
        assert_eq!(notes.len(), 1, "{channel}: {result}");
        assert!(notes[0].contains(channel), "{}", notes[0]);
    }
    Ok(())
}

#[test]
fn every_archive_tool_carries_the_note_and_still_works() -> TestResult {
    let install = Install::standard()?;
    install.add_channel("PTU")?;
    let mut server = install.serve(&[])?;
    let calls = [
        ("list_entries", json!({ "archive": "PTU" })),
        (
            "search_entries",
            json!({ "archive": "PTU", "filter": "readme" }),
        ),
        (
            "read_entry_text",
            json!({ "archive": "PTU", "path": "readme.txt" }),
        ),
    ];
    for (tool, arguments) in calls {
        let result = server.call(tool, &arguments)?;
        assert_ne!(
            result.get("isError"),
            Some(&Value::Bool(true)),
            "{tool}: {result}"
        );
        assert!(
            texts(&result)
                .iter()
                .any(|text| text.contains("confidential")),
            "{tool}: {result}"
        );
    }
    Ok(())
}

#[test]
fn live_and_other_folders_get_no_note() -> TestResult {
    let install = Install::standard()?;
    install.add_channel("Elsewhere")?;
    let mut server = install.serve(&[])?;
    for archive in ["LIVE", "Elsewhere"] {
        let result = server.call("archive_info", &json!({ "archive": archive }))?;
        assert_eq!(texts(&result).len(), 1, "{archive}: {result}");
        assert!(
            !texts(&result)[0].contains("confidential"),
            "{archive}: {result}"
        );
    }
    Ok(())
}
