// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The DataForge tools over a DataForge file on disk in place of an Archive.

use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use crate::forge::{AVENGER_TEXT, dataforge};
use crate::support::{Install, TestResult};

fn with_file() -> TestResult<(Install, String)> {
    let bytes = dataforge(0, 0);
    let install = Install::new(vec![TestEntry::stored("Data\\Game2.dcb", bytes.clone())])?;
    let file = install.work().join("Game2.dcb");
    std::fs::write(&file, bytes)?;
    Ok((install, file.display().to_string()))
}

#[test]
fn every_dataforge_tool_reads_a_file_like_the_archive() -> TestResult {
    let (install, file) = with_file()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let tools = [
        ("get_record", json!({ "guid": AVENGER_TEXT })),
        ("search_records", json!({})),
        ("list_structs", json!({})),
        ("list_enums", json!({})),
        ("run_query", json!({ "query": "from Ship select crew" })),
    ];

    for (tool, arguments) in tools {
        let mut from_file = arguments.clone();
        from_file["archive"] = json!(file);
        let mut from_archive = arguments;
        from_archive["archive"] = json!("LIVE");

        let mut read = server.ok(tool, &from_file)?;
        let mut expected = server.ok(tool, &from_archive)?;
        read["archive"] = json!("");
        expected["archive"] = json!("");

        if matches!(tool, "search_records" | "list_structs" | "list_enums") {
            take_forge_origin(&mut read, "file", &Value::Null)?;
            take_forge_origin(&mut expected, "archiveEntry", &json!(false))?;
        }
        assert_eq!(read, expected, "{tool}");
    }
    Ok(())
}

fn take_forge_origin(result: &mut Value, kind: &str, encryption: &Value) -> TestResult {
    let result = result.as_object_mut().ok_or("search result object")?;
    let fields = result
        .remove("fieldOrigins")
        .ok_or("search field origins")?;
    assert_eq!(
        fields.pointer("/~1structuredContent~1total/containsEncryptedSource"),
        Some(encryption),
        "the total reflects its actual input",
    );
    let provenance = result.remove("provenance").ok_or("search provenance")?;
    assert_eq!(
        provenance.pointer("/sources/0/kind"),
        Some(&json!(kind)),
        "each input retains its own physical source",
    );
    Ok(())
}

#[test]
fn a_cursor_of_a_file_pages_on() -> TestResult {
    let (install, file) = with_file()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let first = server.ok("search_records", &json!({ "archive": file, "limit": 1 }))?;
    let cursor = first["next_cursor"].as_str().ok_or("a next page")?;
    let second = server.ok(
        "search_records",
        &json!({ "archive": file, "limit": 1, "cursor": cursor }),
    )?;

    assert_ne!(first["records"], second["records"], "the next Record");
    Ok(())
}

#[test]
fn only_the_dataforge_tools_take_a_dcb_file() -> TestResult {
    let (install, file) = with_file()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let message = server.error("archive_info", &json!({ "archive": file }))?;

    assert!(message.contains("`.p4k` file"), "{message}");
    assert!(
        message.contains("`.dcb`"),
        "the wording names both: {message}"
    );
    Ok(())
}

#[test]
fn a_file_that_is_no_dataforge_is_an_error() -> TestResult {
    let (install, _) = with_file()?;
    let text = install.work().join("Other.dcb");
    std::fs::write(&text, "not a DataForge at all, just text")?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let message = server.error(
        "get_record",
        &json!({ "archive": text.display().to_string(), "guid": AVENGER_TEXT }),
    )?;

    assert!(!message.is_empty(), "{message}");
    Ok(())
}

#[test]
fn a_truncated_file_is_an_error_naming_the_dataforge() -> TestResult {
    let (install, _) = with_file()?;
    let cut = install.work().join("Cut.dcb");
    std::fs::write(&cut, &dataforge(0, 0)[..200])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let message = server.error(
        "search_records",
        &json!({ "archive": cut.display().to_string() }),
    )?;

    assert!(message.contains("truncated"), "{message}");
    Ok(())
}

#[test]
fn a_changed_file_is_read_again() -> TestResult {
    let (install, file) = with_file()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let before = server.ok("search_records", &json!({ "archive": file }))?;
    std::fs::write(&file, crate::forge::many_items(3))?;

    let after = server.ok("search_records", &json!({ "archive": file }))?;

    assert_ne!(before["total"], after["total"], "the new content");
    assert_eq!(after["total"], 3);
    Ok(())
}
