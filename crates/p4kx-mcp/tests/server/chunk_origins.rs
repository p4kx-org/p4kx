// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestChunkFile, TestEntry};
use serde_json::{Value, json};

use crate::support::{Install, TestResult};

const SELECTED: &str = "Data/selected.cgf";
const OTHER: &str = "Data/other.cgf";
const MATCHING: u32 = 0x00C0_FFEE;
const FILTER: &str = "0x00C0FFEE";
const RESPONSE_LIMIT: usize = 256 * 1024;

#[test]
fn filtered_chunk_pages_bind_the_winning_entry_and_original_row() -> TestResult {
    let file = TestChunkFile::ivo()
        .chunk(0x11, 1, [1; 8])
        .chunk(MATCHING, 2, [2; 8])
        .chunk(0x12, 3, [3; 8])
        .chunk(MATCHING, 4, [4; 8]);
    let plain = TestChunkFile::crch().big_endian_chunk(0x1000, 2, [5; 8]);
    let install = Install::new(vec![
        TestEntry::deflated(SELECTED, file.bytes()).encrypted(),
        TestEntry::stored("data/SELECTED.CGF", plain.bytes()),
        TestEntry::stored(OTHER, plain.bytes()),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let args = json!({"path": "data/SELECTED.CGF", "type": FILTER, "limit": 1});
    let first = server.call("list_chunks", &args)?;
    let first_page = &first["structuredContent"];
    assert_eq!(first_page["chunk_count"], 4);
    assert_eq!(first_page["total"], 2);
    assert_eq!(first_page["path"], SELECTED);
    assert_page(&first, 1, 40, 48)?;
    let second = server.call(
        "list_chunks",
        &json!({
            "path": "data/SELECTED.CGF",
            "type": FILTER,
            "limit": 1,
            "cursor": first_page["next_cursor"],
        }),
    )?;
    assert_page(&second, 3, 72, 80)?;
    assert!(second["structuredContent"].get("next_cursor").is_none());
    assert!(field(&second["structuredContent"], "next_cursor").is_err());

    let plain_page = server.ok("list_chunks", &json!({"path": OTHER}))?;
    marker(field(&plain_page, "chunks/0/offset")?, false);
    marker(field(&plain_page, "chunks/0/byte_order")?, false);
    assert_eq!(plain_page["chunks"][0]["byte_order"], "big");
    assert_eq!(
        uses(&plain_page, "chunks/0/offset")?[0]["location"]["indices"],
        json!(["0", "28", "32"])
    );
    assert_source(&plain_page, OTHER)?;
    Ok(())
}

#[test]
fn chunk_pages_budget_the_retained_origins_and_continue_without_omissions() -> TestResult {
    let count = 170;
    let file = (0..count).fold(TestChunkFile::ivo(), |file, _| {
        file.chunk(MATCHING, 2, [1; 8])
    });
    let install = Install::new(vec![TestEntry::stored(SELECTED, file.bytes())])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut arguments = json!({"path": SELECTED, "limit": 1000});
    let mut next_index = 0;
    loop {
        let reply = server.ok("list_chunks", &arguments)?;
        assert_eq!(reply["total"], count);
        assert!(serde_json::to_vec(&reply)?.len() <= RESPONSE_LIMIT);
        let chunks = reply["chunks"].as_array().ok_or("chunk rows")?;
        assert!(
            !chunks.is_empty(),
            "a retained page must advance the cursor"
        );
        let fields = reply["fieldOrigins"].as_object().ok_or("field origins")?;
        let has_next = reply.get("next_cursor").is_some();
        assert_eq!(fields.len(), 8 + usize::from(has_next) + 10 * chunks.len());
        for (on_page, row) in chunks.iter().enumerate() {
            assert_eq!(row["index"], next_index);
            let address = format!("chunks/{on_page}/offset");
            let original = uses(&reply, &address)?;
            assert_eq!(original.len(), 1);
            assert_eq!(
                original[0]["location"]["indices"][0],
                next_index.to_string()
            );
            next_index += 1;
        }
        assert!(field(&reply, &format!("chunks/{}/offset", chunks.len())).is_err());
        if next_index == chunks.len() {
            assert!(has_next, "the origin metadata forces paging");
        }
        if let Some(cursor) = reply.get("next_cursor") {
            arguments["cursor"] = cursor.clone();
        } else {
            break;
        }
    }
    assert_eq!(next_index, count);
    Ok(())
}

#[test]
fn an_empty_filtered_page_exposes_only_returned_fields_in_the_declared_schema() -> TestResult {
    let install = Install::new(vec![TestEntry::stored(
        SELECTED,
        TestChunkFile::ivo().chunk(0x11, 1, [1; 8]).bytes(),
    )])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let tools = server.tools()?;
    let schema = &tools
        .iter()
        .find(|tool| tool["name"] == "list_chunks")
        .ok_or("chunk tool")?["outputSchema"];
    let required = schema["required"]
        .as_array()
        .ok_or("required output properties")?;
    assert!(required.contains(&json!("fieldOrigins")));
    assert!(required.contains(&json!("provenance")));

    let response = server.call("list_chunks", &json!({"path": SELECTED, "type": FILTER}))?;
    let reply = &response["structuredContent"];
    assert_eq!(reply["total"], 0);
    assert_eq!(reply["chunks"], json!([]));
    assert_eq!(reply["chunk_count"], 1);
    let fields = reply["fieldOrigins"].as_object().ok_or("field origins")?;
    assert_eq!(fields.len(), 8);
    for pointer in fields.keys() {
        assert!(response.pointer(pointer).is_some(), "{pointer}");
    }
    assert!(
        retained(reply)?.iter().all(|site| !matches!(
            site["location"]["field"].as_str(),
            Some("offset" | "offsetColumn" | "decodedExtent")
        )),
        "a filtered-out row supplies no per-row offset or inferred extent"
    );
    Ok(())
}

fn assert_page(response: &Value, ordinal: usize, start: u64, end: u64) -> TestResult {
    let reply = &response["structuredContent"];
    let chunks = reply["chunks"].as_array().ok_or("chunk rows")?;
    let [chunk] = chunks.as_slice() else {
        return Err("expected exactly one selected chunk row".into());
    };
    assert_eq!(chunk["index"], ordinal, "original chunk table ordinal");
    assert_eq!(chunk["type_id"], MATCHING, "the requested chunk type");
    assert_source(reply, SELECTED)?;
    marker(field(reply, "archive")?, false);
    marker(field(reply, "path")?, false);
    marker(field(reply, "container")?, true);
    marker(field(reply, "chunks/0/offset")?, true);
    assert_eq!(
        one_use(reply, "chunks/0/offset")?["location"],
        json!({
            "type": "subresource",
            "namespace": "cryEngineChunkTable",
            "field": "offset",
            "indices": [ordinal.to_string(), start.to_string(), end.to_string()],
        }),
        "original row's half-open offset field range"
    );
    assert_eq!(
        one_use(reply, "chunks/0/type_id_hex")?["role"],
        "conversion",
        "the hex representation is derived from the original type word"
    );
    for aggregate in ["unknown_count", "total", "chunks"] {
        assert!(
            uses(reply, aggregate)?.iter().any(|site| site
                .pointer("/location/field")
                .and_then(Value::as_str)
                == Some("typeColumn")
                && site.pointer("/location/indices") == Some(&json!(["16", "20", "16", "4"]))),
            "{aggregate} needs the original type column"
        );
    }
    assert!(
        retained(reply)?.iter().all(|site| {
            let location = &site["location"];
            location["namespace"] != "cryEngineChunkTable"
                || location["indices"].as_array().is_none_or(|indices| {
                    indices.len() != 3
                        || indices
                            .first()
                            .and_then(Value::as_str)
                            .and_then(|ordinal| ordinal.parse::<usize>().ok())
                            == Some(ordinal)
                })
        }),
        "other pages and excluded rows must have no per-row source uses"
    );
    for pointer in reply["fieldOrigins"]
        .as_object()
        .ok_or("field origins")?
        .keys()
    {
        assert!(response.pointer(pointer).is_some(), "{pointer}");
    }
    Ok(())
}

fn assert_source(reply: &Value, path: &str) -> TestResult {
    let sources = reply
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("sources")?;
    let [source] = sources.as_slice() else {
        return Err("expected only the admitted input".into());
    };
    assert_eq!(
        source.pointer("/path/value").and_then(Value::as_str),
        Some(path),
        "physical Entry name"
    );
    Ok(())
}

fn marker(origin: &Value, expected: bool) {
    assert_eq!(
        origin["containsEncryptedSource"], expected,
        "field encryption"
    );
    assert_eq!(origin["sourceSetComplete"], true, "complete contributors");
}

fn field<'reply>(reply: &'reply Value, path: &str) -> TestResult<&'reply Value> {
    reply["fieldOrigins"]
        .get(format!("/structuredContent/{path}"))
        .ok_or_else(|| format!("missing field origin: {path}").into())
}

fn retained(reply: &Value) -> TestResult<&[Value]> {
    reply
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "registered source uses".into())
}

fn one_use<'reply>(reply: &'reply Value, path: &str) -> TestResult<&'reply Value> {
    match uses(reply, path)?.as_slice() {
        [site] => Ok(*site),
        _ => Err(format!("expected one source use at {path}").into()),
    }
}

fn uses<'reply>(reply: &'reply Value, path: &str) -> TestResult<Vec<&'reply Value>> {
    let sites = retained(reply)?;
    field(reply, path)?["sourceUsesRefs"]
        .as_array()
        .ok_or("source use references")?
        .iter()
        .map(|reference| {
            sites
                .iter()
                .find(|site| &site["id"] == reference)
                .ok_or_else(|| "unresolved source use".into())
        })
        .collect()
}
