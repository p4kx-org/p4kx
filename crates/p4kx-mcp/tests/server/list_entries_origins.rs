// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestContainer, TestEntry};
use serde_json::{Value, json};

use crate::support::{Install, TestResult};

#[test]
fn folder_spelling_and_count_use_different_original_inputs() -> TestResult {
    let install = Install::new(vec![
        TestEntry::stored("Data/Beta/q.txt", b"beta"),
        TestEntry::stored("Data/ALPHA/same.txt", b"first"),
        TestEntry::deflated("Data/ALPHA/same.txt", b"second").encrypted(),
        TestEntry::stored("data/Alpha/0.txt", b"first path spelling"),
        TestEntry::stored("Data/z.txt", b"top file"),
        TestEntry::stored("Other/hidden.txt", b"other"),
    ])?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    let reply = server.ok("list_entries", &json!({"folder":"dAtA", "limit":1}))?;
    assert_eq!(reply["folder"], "dAtA");
    assert_eq!(reply["total"], 3);
    assert_eq!(reply["items"][0]["path"], "data/Alpha");
    assert_eq!(reply["items"][0]["entry_count"], 3);
    let path = field(&reply, "/items/0/path");
    assert_eq!(path["containsEncryptedSource"], false);
    assert_eq!(entry_owners(&reply, path)?, vec![3]);
    let count = field(&reply, "/items/0/entry_count");
    assert_eq!(count["containsEncryptedSource"], false);
    assert_eq!(count["sourceSetComplete"], true);
    let survey = survey(&reply, count)?;
    assert_eq!(
        survey["location"],
        json!({
            "type":"archiveDirectoryTreePaths", "startEntryIndex":"0", "endEntryIndex":"3"
        })
    );
    assert_eq!(field(&reply, "/folder")["sourceUsesRefs"], json!([]));
    assert_eq!(field(&reply, "/total"), field(&reply, "/next_cursor"));
    let cursor = reply["next_cursor"].as_str().ok_or("folder cursor")?;
    let next = server.ok(
        "list_entries",
        &json!({"folder":"dAtA", "limit":2, "cursor":cursor}),
    )?;
    assert_eq!(next["items"][0]["path"], "Data/Beta");
    assert_eq!(next["items"][1]["path"], "Data/z.txt");
    assert_eq!(next["total"], reply["total"]);
    assert!(next.get("next_cursor").is_none());
    let duplicates = server.ok("list_entries", &json!({"folder":"Data/alpha"}))?;
    let mut owners = Vec::new();
    for ordinal in [1, 2] {
        let address = format!("/items/{ordinal}/encrypted");
        let origin = field(&duplicates, &address);
        assert_eq!(origin["containsEncryptedSource"], false);
        owners.extend(entry_owners(&duplicates, origin)?);
    }
    owners.sort_unstable();
    assert_eq!(owners, vec![1, 2]);
    Ok(())
}

#[test]
fn missing_and_empty_container_folders_keep_their_selected_directory_proof() -> TestResult {
    let nested = TestContainer::new()
        .entry(TestEntry::stored("Data/Inside/a.txt", b"plain"))
        .entry(TestEntry::stored("Data/Other/b.txt", b"plain"))
        .build();
    let empty = TestContainer::new().build();
    let install = Install::new(vec![
        TestEntry::deflated("selected.socpak", nested).encrypted(),
        TestEntry::deflated("empty.socpak", empty).encrypted(),
        TestEntry::stored("Other/silent.txt", b"unused"),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let missing = server.ok(
        "list_entries",
        &json!({"folder":"selected.socpak::Data/missing"}),
    )?;
    assert_eq!(missing["total"], 0);
    assert_eq!(field(&missing, "/folder")["containsEncryptedSource"], false);
    assert_eq!(field(&missing, "/total")["containsEncryptedSource"], true);
    assert_eq!(field(&missing, "/total")["sourceSetComplete"], true);
    assert!(
        receipts(&missing, field(&missing, "/total"))?
            .iter()
            .any(|item| {
                item["location"]["namespace"] == "archiveEntryDecodedPath"
                    && item["location"]["field"] == "comparedFolderName"
            })
    );
    assert!(!missing["provenance"].to_string().contains("silent.txt"));
    let empty = server.ok("list_entries", &json!({"folder":"empty.socpak::"}))?;
    assert_eq!(empty["total"], 0);
    assert_eq!(empty["items"], json!([]));
    assert_eq!(field(&empty, "/total")["containsEncryptedSource"], true);
    assert_eq!(field(&empty, "/total")["sourceSetComplete"], true);
    assert_eq!(field(&empty, "/items")["sourceUsesRefs"], json!([]));
    assert!(
        receipts(&empty, field(&empty, "/total"))?
            .iter()
            .any(|item| {
                item["location"]["namespace"] == "containerDirectory"
                    && item["location"]["field"] == "zipEnd"
            })
    );
    Ok(())
}

fn field<'a>(response: &'a Value, path: &str) -> &'a Value {
    response
        .get("fieldOrigins")
        .and_then(|fields| fields.get(format!("/structuredContent{path}")))
        .unwrap_or(&Value::Null)
}

fn receipts<'a>(response: &'a Value, field: &Value) -> TestResult<Vec<&'a Value>> {
    let registered = response
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("field source uses")?
        .iter()
        .map(|reference| {
            registered
                .iter()
                .find(|item| item["id"] == *reference)
                .ok_or_else(|| "registered source use".into())
        })
        .collect()
}

fn survey<'a>(response: &'a Value, field: &Value) -> TestResult<&'a Value> {
    receipts(response, field)?
        .into_iter()
        .find(|usage| {
            usage.pointer("/location/type").and_then(Value::as_str)
                == Some("archiveDirectoryTreePaths")
        })
        .ok_or_else(|| "path-order directory survey".into())
}

fn entry_owners(response: &Value, field: &Value) -> TestResult<Vec<u64>> {
    let sources = response
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("registered sources")?;
    let mut indices = Vec::new();
    for usage in receipts(response, field)? {
        let source = sources
            .iter()
            .find(|source| source["id"] == usage["source"])
            .ok_or("original physical source")?;
        if let Some(index) = source["entryIndex"].as_u64() {
            indices.push(index);
        }
    }
    indices.sort_unstable();
    indices.dedup();
    Ok(indices)
}

#[test]
fn a_large_folder_uses_one_complete_range_and_an_empty_root_uses_zero() -> TestResult {
    let install = Install::new(
        (0..2500)
            .map(|index| TestEntry::stored(format!("Data/Many/f{index:04}.txt"), b"synthetic"))
            .collect(),
    )?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    let page = server.ok("list_entries", &json!({"folder":"Data", "limit":1}))?;
    assert_eq!(page["items"][0]["entry_count"], 2500);
    let count = field(&page, "/items/0/entry_count");
    assert_eq!(count["sourceSetComplete"], true);
    assert_eq!(receipts(&page, count)?.len(), 1);
    assert_eq!(survey(&page, count)?["location"]["endEntryIndex"], "2500");
    assert!(serde_json::to_vec(&page)?.len() < 32 * 1024);
    let empty = Install::new(Vec::new())?;
    let mut empty = empty.serve_without_key(&["--archive", "LIVE"])?;
    let root = empty.ok("list_entries", &json!({}))?;
    assert_eq!(root["total"], 0);
    assert_eq!(field(&root, "/total")["containsEncryptedSource"], false);
    assert_eq!(field(&root, "/total")["sourceSetComplete"], true);
    assert_eq!(
        survey(&root, field(&root, "/total"))?["location"],
        json!({
            "type":"archiveDirectoryTreePaths", "startEntryIndex":"0", "endEntryIndex":"0"
        })
    );
    let tools = empty.tools()?;
    let list = tools
        .iter()
        .find(|tool| tool["name"] == "list_entries")
        .ok_or("list tool")?;
    let required = list["outputSchema"]["required"]
        .as_array()
        .ok_or("required fields")?;
    assert!(required.contains(&json!("fieldOrigins")) && required.contains(&json!("provenance")));
    Ok(())
}
