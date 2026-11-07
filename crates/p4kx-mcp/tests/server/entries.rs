// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use crate::support::{Install, Server, TestResult};

fn metadata(owner: usize, method: u16) -> Value {
    json!({
        "owner_entry_index": owner, "original_directory_ordinal": owner.to_string(),
        "dos_timestamp": {"raw_date": 0, "raw_time": 0, "calendar": {"state": "invalid"}},
        "record_format": "zip_central_directory", "raw_compression_method": method,
        "zip_header": {"raw_version_made_by": 45, "raw_version_needed": 45,
            "general_purpose_flags": 0, "disk_start": 65535,
            "internal_attributes": 0, "external_attributes": 0},
        "raw_v2_encryption_word": null
    })
}

fn paths(items: &Value) -> Vec<String> {
    items
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["path"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn collect_pages(
    server: &mut Server,
    tool: &str,
    arguments: &Value,
    key: &str,
) -> TestResult<Vec<String>> {
    let mut all = Vec::new();
    let mut arguments = arguments.as_object().cloned().unwrap_or_default();
    loop {
        let page = server.ok(tool, &Value::Object(arguments.clone()))?;
        all.extend(paths(page.get(key).unwrap_or(&Value::Null)));
        match page.get("next_cursor").and_then(Value::as_str) {
            Some(cursor) => arguments.insert("cursor".to_owned(), json!(cursor)),
            None => return Ok(all),
        };
    }
}

#[test]
fn the_top_level_lists_folders_then_entries() -> TestResult {
    let install = Install::standard()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let top = server.ok("list_entries", &json!({}))?;
    assert_eq!(
        top,
        json!({
            "archive": install.archive().display().to_string(),
            "folder": "",
            "total": 2,
            "items": [
                { "kind": "folder", "path": "Data", "entry_count": 5 },
                {
                    "kind": "entry",
                    "path": "readme.txt",
                    "compression": top["items"][1]["compression"],
                    "encrypted": false,
                    "compressed_size": 5,
                    "size": 5,
                    "metadata": metadata(5, 0)
                }
            ]
        }),
        "the top level"
    );
    Ok(())
}

#[test]
fn a_folder_lists_its_subfolders_by_name_and_its_entries_in_path_order() -> TestResult {
    let install = Install::standard()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let data = server.ok("list_entries", &json!({ "folder": "data/" }))?;
    assert_eq!(data["folder"], "data", "the folder as asked, trimmed");
    assert_eq!(
        paths(&data["items"]),
        ["Data/Libs", "Data/Objects", "Data/Game2.dcb"],
        "folders first, then Entries"
    );
    assert_eq!(data["items"][0]["entry_count"], 3, "Entries below Libs");
    let config = server.ok("list_entries", &json!({ "folder": r"DATA\libs\Config" }))?;
    assert_eq!(
        paths(&config["items"]),
        ["Data/Libs/Config/a.xml", "Data/Libs/Config/b.xml"],
        "case and separator kind do not matter"
    );
    let other = server.ok("list_entries", &json!({ "folder": "Data/Libs/Other" }))?;
    assert_eq!(other["items"][0]["encrypted"], true, "the encrypted Entry");
    let none = server.ok("list_entries", &json!({ "folder": "Data/Missing" }))?;
    assert_eq!(none["total"], 0, "an unknown folder is empty");
    Ok(())
}

fn many(count: usize) -> Vec<TestEntry> {
    (0..count)
        .map(|index| TestEntry::stored(format!("Data/Many/{index:04}.xml"), b"x".to_vec()))
        .collect()
}

#[test]
fn listings_page_with_a_cursor_until_every_item_is_seen_once() -> TestResult {
    let install = Install::new(many(25))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.ok(
        "list_entries",
        &json!({ "folder": "Data/Many", "limit": 10 }),
    )?;
    assert_eq!(first["total"], 25, "the total of the listing");
    assert_eq!(
        first["items"].as_array().map(Vec::len),
        Some(10),
        "one page"
    );
    let listed = collect_pages(
        &mut server,
        "list_entries",
        &json!({ "folder": "Data/Many", "limit": 10 }),
        "items",
    )?;
    let expected: Vec<String> = (0..25)
        .map(|index| format!("Data/Many/{index:04}.xml"))
        .collect();
    assert_eq!(listed, expected, "every Entry once, in order");
    let found = collect_pages(
        &mut server,
        "search_entries",
        &json!({ "filter": "*.xml", "limit": 7 }),
        "entries",
    )?;
    assert_eq!(found, expected, "search pages the same way");
    Ok(())
}

#[test]
fn the_page_size_is_capped_at_one_thousand() -> TestResult {
    let install = Install::new(many(1005))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let page = server.ok(
        "search_entries",
        &json!({ "filter": "Many", "limit": 5000 }),
    )?;
    assert_eq!(page["total"], 1005, "every match counted");
    assert_eq!(
        page["entries"].as_array().map(Vec::len),
        Some(1000),
        "at most 1000"
    );
    let cursor = page["next_cursor"].as_str().unwrap_or_default();
    let rest = server.ok(
        "search_entries",
        &json!({ "filter": "Many", "limit": 5000, "cursor": cursor }),
    )?;
    assert_eq!(
        rest["entries"].as_array().map(Vec::len),
        Some(5),
        "the next page holds the rest"
    );
    assert!(rest.get("next_cursor").is_none(), "the last page");
    let default = server.ok("search_entries", &json!({ "filter": "Many" }))?;
    assert_eq!(
        default["entries"].as_array().map(Vec::len),
        Some(100),
        "100 by default"
    );
    Ok(())
}

#[test]
fn search_follows_the_filter_rules() -> TestResult {
    let install = Install::standard()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let glob = server.ok("search_entries", &json!({ "filter": r"data\libs\*.XML" }))?;
    assert_eq!(
        glob,
        json!({
            "archive": install.archive().display().to_string(),
            "filter": r"data\libs\*.XML",
            "total": 2,
            "entries": [
                {
                    "path": "Data/Libs/Config/a.xml",
                    "compression": glob["entries"][0]["compression"],
                    "encrypted": false,
                    "compressed_size": 4,
                    "size": 4,
                    "metadata": metadata(0, 0)
                },
                {
                    "path": "Data/Libs/Config/b.xml",
                    "compression": glob["entries"][1]["compression"],
                    "encrypted": false,
                    "compressed_size": glob["entries"][1]["compressed_size"],
                    "size": 80,
                    "metadata": metadata(1, 8)
                }
            ]
        }),
        "a glob over the whole path"
    );
    let substring = server.ok("search_entries", &json!({ "filter": "OBJECTS/ship" }))?;
    assert_eq!(
        paths(&substring["entries"]),
        ["Data/Objects/ship.cgf"],
        "a substring"
    );
    let glob_whole = server.ok("search_entries", &json!({ "filter": "*.xm?" }))?;
    assert_eq!(glob_whole["total"], 2, "? matches one character");
    let nothing = server.ok("search_entries", &json!({ "filter": "nothing" }))?;
    assert_eq!(
        nothing,
        json!({
            "archive": install.archive().display().to_string(),
            "filter": "nothing",
            "total": 0,
            "entries": []
        }),
        "no match is no error"
    );
    Ok(())
}

#[test]
fn bad_cursors_and_oversized_filters_are_tool_errors() -> TestResult {
    let install = Install::standard()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    for cursor in ["abc", "-1", "99", ""] {
        let message = server.error("search_entries", &json!({ "filter": "", "cursor": cursor }))?;
        assert!(message.contains("cursor"), "{cursor:?}: {message}");
    }
    let page = server.ok("search_entries", &json!({ "filter": "Data", "limit": 2 }))?;
    let cursor = page["next_cursor"].as_str().unwrap_or_default();
    let message = server.error("list_entries", &json!({ "cursor": cursor }))?;
    assert!(message.contains("cursor"), "another listing: {message}");
    let message = server.error(
        "search_entries",
        &json!({ "filter": "Libs", "limit": 2, "cursor": cursor }),
    )?;
    assert!(message.contains("cursor"), "another filter: {message}");
    let message = server.error("search_entries", &json!({ "filter": "x".repeat(2000) }))?;
    assert!(
        message.contains("longer than"),
        "a capped filter: {message}"
    );
    let message = server.error("search_entries", &json!({ "filter": "*a".repeat(40) }))?;
    assert!(
        message.contains("40 wildcards"),
        "a Filter with too many wildcards: {message}"
    );
    let message = server.error("list_entries", &json!({ "folder": "x".repeat(2000) }))?;
    assert!(
        message.contains("longer than"),
        "a capped folder: {message}"
    );
    Ok(())
}
