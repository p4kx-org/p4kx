// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};
use serde_json::{Value, json};

use crate::support::{Install, TestResult};

#[test]
fn search_uses_physical_directory_fields_without_reading_the_encrypted_leaf() -> TestResult {
    let install = Install::new(vec![
        TestEntry::stored("Data/alpha.txt", b"clear"),
        TestEntry::deflated("Data/secret.txt", b"secret").encrypted(),
        TestEntry::stored("Data/skip.bin", b"skipped"),
    ])?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    let first = server.ok("search_entries", &json!({"filter":"*.txt", "limit":1}))?;
    assert_eq!(first["total"], 2);
    assert_eq!(first["entries"][0]["path"], "Data/alpha.txt");
    let cursor = first["next_cursor"].as_str().ok_or("continuation cursor")?;
    let second = server.ok(
        "search_entries",
        &json!({"filter":"*.txt", "limit":1, "cursor": cursor}),
    )?;
    assert_eq!(second["entries"][0]["encrypted"], true);
    for (response, selected) in [(&first, 0), (&second, 1)] {
        let origin = field(response, "/entries/0/encrypted");
        assert_eq!(origin["containsEncryptedSource"], false);
        assert_eq!(origin["sourceSetComplete"], true);
        let uses = uses(response, origin)?;
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0]["scope"], "archiveDirectoryMetadata");
        assert_eq!(owners(response, &uses)?, vec![selected]);
        for field_name in [
            "/entries/0/path",
            "/entries/0/compressed_size",
            "/entries/0/size",
            "/entries/0/metadata",
            "/entries/0/metadata/dos_timestamp/raw_date",
            "/entries/0/metadata/raw_v2_encryption_word",
        ] {
            assert_eq!(
                field(response, field_name)["containsEncryptedSource"],
                false
            );
        }
        let total = field(response, "/total");
        assert_eq!(total["containsEncryptedSource"], false);
        assert_eq!(total["sourceSetComplete"], true);
        let survey = self::uses(response, total)?;
        assert_eq!(survey.len(), 1);
        let site = survey.first().ok_or("root survey")?;
        assert_eq!(site["location"]["endEntryIndex"], "3");
        assert_eq!(receipt_source(response, site)?["admittedEntryCount"], "3");
        let items = field(response, "/entries");
        assert!(
            owners(response, &self::uses(response, items)?)?
                .iter()
                .all(|&id| id == selected)
        );
    }
    Ok(())
}

#[test]
fn search_inherits_encrypted_container_for_rows_and_zero_matches() -> TestResult {
    let nested = TestContainer::new()
        .entry(TestEntry::stored("Data/inside.txt", b"clear"))
        .build();
    let install = Install::new(vec![
        TestEntry::deflated("Data/selected.socpak", nested.clone()).encrypted(),
        TestEntry::stored("Data/unused.socpak", nested),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let selected = server.ok(
        "search_entries",
        &json!({"container":"Data/selected.socpak", "filter":"inside"}),
    )?;
    assert_eq!(selected["entries"][0]["encrypted"], false);
    assert_eq!(
        field(&selected, "/container")["containsEncryptedSource"],
        false
    );
    for name in ["/total", "/entries/0/path", "/entries/0/size"] {
        assert_eq!(
            field(&selected, name)["containsEncryptedSource"],
            true,
            "{name}"
        );
    }
    let none = server.ok(
        "search_entries",
        &json!({"container":"Data/selected.socpak", "filter":"no match"}),
    )?;
    assert_eq!(none["total"], 0);
    assert_eq!(field(&none, "/total")["containsEncryptedSource"], true);
    assert_eq!(field(&none, "/entries")["containsEncryptedSource"], false);
    assert!(!none["provenance"].to_string().contains("unused.socpak"));
    Ok(())
}

fn field<'a>(response: &'a Value, path: &str) -> &'a Value {
    response
        .get("fieldOrigins")
        .and_then(|fields| fields.get(format!("/structuredContent{path}")))
        .unwrap_or(&Value::Null)
}

fn uses<'a>(response: &'a Value, field: &Value) -> TestResult<Vec<&'a Value>> {
    let registered = response
        .pointer("/provenance/uses")
        .unwrap_or(&Value::Null)
        .as_array()
        .ok_or("source uses")?;
    field["sourceUsesRefs"]
        .as_array()
        .ok_or("field source uses")?
        .iter()
        .map(|reference| {
            registered
                .iter()
                .find(|item| &item["id"] == reference)
                .ok_or_else(|| "used source".into())
        })
        .collect()
}

fn owners(response: &Value, uses: &[&Value]) -> TestResult<Vec<u64>> {
    let mut ids = Vec::new();
    for usage in uses {
        let source = receipt_source(response, usage)?;
        let index = source["entryIndex"]
            .as_u64()
            .ok_or("physical entry index")?;
        ids.push(index);
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

#[test]
fn an_empty_encrypted_container_supplies_the_count_via_its_zip_end() -> TestResult {
    let empty = TestContainer::new().build();
    let install = Install::new(vec![TestEntry::deflated("empty.socpak", empty).encrypted()])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let reply = server.ok(
        "search_entries",
        &json!({"container":"empty.socpak", "filter":""}),
    )?;
    assert_eq!(reply["total"], 0);
    let total = field(&reply, "/total");
    assert_eq!(total["containsEncryptedSource"], true);
    assert_eq!(total["sourceSetComplete"], true);
    assert!(uses(&reply, total)?.iter().any(|usage| {
        usage["scope"] == "payload"
            && usage["location"]["field"] == "zipEnd"
            && usage["location"]["indices"] == json!(["0", "22"])
    }));
    assert_eq!(field(&reply, "/entries")["sourceUsesRefs"], json!([]));
    Ok(())
}

#[test]
fn duplicate_paths_keep_distinct_owners_and_output_schema_requires_origins() -> TestResult {
    let install = Install::new(vec![
        TestEntry::stored("same.txt", b"first"),
        TestEntry::stored("same.txt", b"second"),
    ])?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    let reply = server.ok("search_entries", &json!({"filter":"same"}))?;
    assert_eq!(reply["total"], 2);
    for ordinal in 0..2 {
        let pointer = format!("/entries/{ordinal}/path");
        assert_eq!(
            owners(&reply, &uses(&reply, field(&reply, &pointer))?)?,
            vec![ordinal]
        );
    }
    let tools = server.tools()?;
    let search = tools
        .iter()
        .find(|tool| tool["name"] == "search_entries")
        .ok_or("search tool")?;
    let required = search["outputSchema"]["required"]
        .as_array()
        .ok_or("required fields")?;
    assert!(required.contains(&json!("fieldOrigins")) && required.contains(&json!("provenance")));
    Ok(())
}

#[test]
fn large_root_survey_is_complete_with_one_directory_source_use() -> TestResult {
    let entries =
        (0..2000).map(|index| TestEntry::stored(format!("Data/f{index:04}.txt"), b"synthetic"));
    let install = Install::new(entries.collect())?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    let reply = server.ok("search_entries", &json!({"filter":"*.txt", "limit":2}))?;
    assert_eq!(reply["total"], 2000);
    assert_eq!(reply["entries"].as_array().map(Vec::len), Some(2));
    assert!(reply["next_cursor"].as_str().is_some());
    let total = field(&reply, "/total");
    assert_eq!(total["sourceSetComplete"], true);
    assert_eq!(total["containsEncryptedSource"], false);
    assert_eq!(field(&reply, "/next_cursor"), total);
    let receipts = uses(&reply, total)?;
    assert_eq!(receipts.len(), 1);
    let source = receipt_source(&reply, receipts.first().ok_or("root survey")?)?;
    assert_eq!(source["kind"], "archiveDirectory");
    assert_eq!(source["admittedEntryCount"], "2000");
    assert_eq!(
        receipts[0]["location"],
        json!({"type":"archiveDirectoryPaths","startEntryIndex":"0","endEntryIndex":"2000"})
    );
    assert_eq!(receipts[0]["scope"], "archiveDirectoryMetadata");
    assert_eq!(receipts[0]["role"], "selector");
    assert!(serde_json::to_vec(&reply)?.len() < 32 * 1024);
    assert_eq!(field(&reply, "/entries/1/size")["sourceSetComplete"], true);
    assert_eq!(
        owners(&reply, &uses(&reply, field(&reply, "/entries/1/size"))?)?,
        vec![1]
    );
    Ok(())
}

#[test]
fn a_v2_encryption_word_is_a_clear_directory_field_even_without_a_key() -> TestResult {
    let install = Install::new(Vec::new())?;
    ArchiveBuilder::v2()
        .entry(TestEntry::deflated("raw.txt", b"secret").encrypted())
        .write_to(install.archive())?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    let reply = server.ok("search_entries", &json!({"filter":"raw"}))?;
    assert_eq!(reply["entries"][0]["encrypted"], true);
    assert_eq!(reply["entries"][0]["metadata"]["raw_v2_encryption_word"], 1);
    assert_eq!(reply["entries"][0]["metadata"]["zip_header"], Value::Null);
    let raw_word = field(&reply, "/entries/0/metadata/raw_v2_encryption_word");
    assert_eq!(raw_word["containsEncryptedSource"], false);
    assert_eq!(raw_word["sourceSetComplete"], true);
    let sources = uses(&reply, raw_word)?;
    assert_eq!(sources.len(), 1);
    assert!(sources.iter().all(|usage| {
        usage["scope"] == "archiveDirectoryMetadata"
            && usage["location"]["field"] == "raw_v2_encryption_word"
    }));
    Ok(())
}

#[test]
fn an_empty_root_retains_its_original_directory_as_a_complete_survey() -> TestResult {
    let install = Install::new(Vec::new())?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    let reply = server.ok("search_entries", &json!({"filter":"empty"}))?;
    assert_eq!(reply["total"], 0);
    let total = field(&reply, "/total");
    assert_eq!(total["containsEncryptedSource"], false);
    assert_eq!(total["sourceSetComplete"], true);
    let receipts = uses(&reply, total)?;
    assert_eq!(receipts.len(), 1);
    let source = receipt_source(&reply, receipts.first().ok_or("root survey")?)?;
    assert_eq!(source["admittedEntryCount"], "0");
    assert_eq!(receipts[0]["location"]["startEntryIndex"], "0");
    assert_eq!(receipts[0]["location"]["endEntryIndex"], "0");
    Ok(())
}

fn receipt_source<'a>(reply: &'a Value, usage: &Value) -> TestResult<&'a Value> {
    reply
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .and_then(|sources| {
            sources
                .iter()
                .find(|source| source["id"] == usage["source"])
        })
        .ok_or_else(|| "source used by field".into())
}
