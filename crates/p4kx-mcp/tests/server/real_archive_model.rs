// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::support::{Install, Server, TestResult};

const SLOW: Duration = Duration::from_mins(5);
const CANDIDATES: u64 = 300;

fn count(value: &Value, pointer: &str) -> Option<u64> {
    value.pointer(pointer).and_then(Value::as_u64)
}

fn real_server(install: &Install) -> TestResult<Server> {
    let archive = p4kx_testkit::real_archive_path()?;
    let path = archive.display().to_string();
    let mut server = install.serve_real(&["--archive", &path])?;
    server.wait_up_to(SLOW);
    Ok(server)
}

fn model_paths(server: &mut Server, filter: &str) -> TestResult<Vec<String>> {
    let found = server.ok(
        "search_entries",
        &json!({ "filter": filter, "limit": CANDIDATES }),
    )?;
    Ok(found
        .get("entries")
        .and_then(Value::as_array)
        .map_or_default(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("path").and_then(Value::as_str).map(str::to_owned))
                .collect()
        }))
}

fn first_model(server: &mut Server, filter: &str, bones: bool) -> TestResult<(String, Value)> {
    for path in model_paths(server, filter)? {
        let result = server.call("get_model_info", &json!({ "path": path }))?;
        if result.get("isError") == Some(&Value::Bool(true)) {
            continue;
        }
        let Some(info) = result.get("structuredContent") else {
            continue;
        };
        let has_bones = count(info, "/counts/bones").is_some_and(|n| n > 0);
        if count(info, "/counts/meshes").is_some_and(|n| n > 0) && has_bones == bones {
            return Ok((path, info.clone()));
        }
    }
    Err(format!("no readable model among the first {CANDIDATES} of {filter}").into())
}

struct Walk {
    total: u64,
    items: usize,
    pages: usize,
}

fn walk(server: &mut Server, tool: &str, base: &Value, list: &str, limit: u64) -> TestResult<Walk> {
    let mut arguments = base.as_object().cloned().unwrap_or_default();
    arguments.insert("limit".to_owned(), json!(limit));
    let mut walked = Walk {
        total: 0,
        items: 0,
        pages: 0,
    };
    loop {
        let page = server.ok(tool, &Value::Object(arguments.clone()))?;
        walked.total = count(&page, "/total").ok_or("total")?;
        walked.items += page
            .get(list)
            .and_then(Value::as_array)
            .ok_or("the listed items")?
            .len();
        walked.pages += 1;
        assert!(walked.pages < 10_000, "paging terminates");
        let Some(cursor) = page.get("next_cursor").and_then(Value::as_str) else {
            return Ok(walked);
        };
        arguments.insert("cursor".to_owned(), json!(cursor));
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(clippy::print_stdout, reason = "the measurements are the output")]
fn list_chunks_reads_and_pages_a_real_model() -> TestResult {
    let install = Install::new(Vec::new())?;
    let mut server = real_server(&install)?;
    let (model, _) = first_model(&mut server, "*.cgf", false)?;

    let started = Instant::now();
    let listing = server.ok("list_chunks", &json!({ "path": model, "limit": 2 }))?;
    println!("list_chunks of {model}: {:.2?}", started.elapsed());
    let chunks = count(&listing, "/chunk_count").ok_or("chunk_count")?;
    assert!(chunks >= 3, "a model has several chunks: {chunks}");
    assert_eq!(count(&listing, "/total"), Some(chunks), "unfiltered total");
    assert_eq!(listing["chunks"].as_array().map(Vec::len), Some(2), "page");
    assert!(listing["next_cursor"].is_string(), "more chunks follow");
    assert!(
        listing["container"]
            .as_str()
            .is_some_and(|form| !form.is_empty()),
        "the container form: {listing}"
    );

    let walked = walk(
        &mut server,
        "list_chunks",
        &json!({ "path": model }),
        "chunks",
        2,
    )?;
    assert_eq!(
        walked.items as u64, chunks,
        "the pages add up to every chunk"
    );
    assert!(walked.pages >= 2, "the listing pages");
    let named = listing["chunks"]
        .as_array()
        .and_then(|page| page.iter().find_map(|chunk| chunk["type"].as_str()))
        .ok_or("a named chunk on the first page")?;
    let filtered = server.ok("list_chunks", &json!({ "path": model, "type": named }))?;
    assert!(
        count(&filtered, "/total").is_some_and(|n| (1..=chunks).contains(&n)),
        "the type filter narrows: {filtered}"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(clippy::print_stdout, reason = "the measurements are the output")]
fn model_info_and_items_agree_on_a_real_model() -> TestResult {
    let install = Install::new(Vec::new())?;
    let mut server = real_server(&install)?;
    let started = Instant::now();
    let (model, info) = first_model(&mut server, "*.cgf", false)?;
    println!(
        "get_model_info of {model}: found in {:.2?}",
        started.elapsed()
    );
    assert!(
        info["lods"]
            .as_array()
            .and_then(|lods| lods.first())
            .is_some_and(|lod| {
                count(lod, "/vertices").is_some_and(|n| n > 0)
                    && count(lod, "/triangles").is_some_and(|n| n > 0)
            }),
        "a level of detail with geometry: {info}"
    );

    let base = json!({ "path": model });
    for section in ["nodes", "materials", "chunks", "meshes"] {
        let mut arguments = base.clone();
        arguments["section"] = json!(section);
        let started = Instant::now();
        let walked = walk(&mut server, "list_model_items", &arguments, "items", 3)?;
        println!(
            "list_model_items {section}: {} items, {} pages, {:.2?}",
            walked.total,
            walked.pages,
            started.elapsed()
        );
        assert_eq!(walked.items as u64, walked.total, "section {section} pages");
        if section != "meshes" {
            assert_eq!(
                Some(walked.total),
                count(&info, &format!("/counts/{section}")),
                "section {section} equals the summary count"
            );
        }
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(clippy::print_stdout, reason = "the measurements are the output")]
fn the_model_tools_read_a_real_skeleton() -> TestResult {
    let install = Install::new(Vec::new())?;
    let mut server = real_server(&install)?;
    let (model, info) = first_model(&mut server, "*.skin", true)?;
    let bones = count(&info, "/counts/bones").ok_or("bones")?;
    println!("skinned model {model}: {bones} bones");
    let arguments = json!({ "path": model, "section": "bones" });
    let walked = walk(&mut server, "list_model_items", &arguments, "items", 20)?;
    assert_eq!(
        walked.total, bones,
        "the bone list equals the summary count"
    );
    assert_eq!(walked.items as u64, bones, "the pages add up to every bone");
    Ok(())
}
