// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::forge::{AEGS_TEXT, AVENGER_TEXT, dataforge, install, many_items};
use crate::support::{Server, TestResult, key_order, member};

fn texts(items: &Value, key: &str) -> Vec<String> {
    items.as_array().map_or_default(|items| {
        items
            .iter()
            .filter_map(|item| item[key].as_str().map(str::to_owned))
            .collect()
    })
}

/// Calls `tool` page by page and returns every item under `key`.
pub(crate) fn every_page(
    server: &mut Server,
    tool: &str,
    arguments: &Value,
    key: &str,
) -> TestResult<(Vec<Value>, usize)> {
    let mut all = Vec::new();
    let mut pages = 0;
    let mut arguments = arguments.as_object().cloned().unwrap_or_default();
    loop {
        let page = server.ok(tool, &Value::Object(arguments.clone()))?;
        pages += 1;
        all.extend(
            page.get(key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        );
        match page.get("next_cursor").and_then(Value::as_str) {
            Some(cursor) => arguments.insert("cursor".to_owned(), json!(cursor)),
            None => return Ok((all, pages)),
        };
    }
}

#[test]
fn a_record_by_guid_is_its_canonical_record_json() -> TestResult {
    let install = install(dataforge(0, 0))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let result = server.ok(
        "get_record",
        &json!({ "guid": AVENGER_TEXT.to_uppercase() }),
    )?;
    assert_eq!(result["total"], 1, "one Record per GUID");
    assert!(result.get("next_cursor").is_none(), "one page");
    assert_eq!(
        result["records"][0],
        json!({
            "$type": "Ship",
            "$guid": AVENGER_TEXT,
            "$path": "libs/foundry/records/entities/spaceships/aegs_avenger",
            "$name": "Ship.Avenger",
            "mass": 1000,
            "size": "Medium",
            "maker": {
                "guid": AEGS_TEXT,
                "path": "libs/foundry/records/manufacturers/aegs",
                "type": "Manufacturer"
            },
            "crew": 2,
            "tags": ["fighter", "Avenger"]
        }),
        "the Record JSON with the Reference as a link"
    );
    Ok(())
}

#[test]
fn a_record_path_gives_every_record_at_it_in_definition_order() -> TestResult {
    let install = install(dataforge(0, 0))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let result = server.ok(
        "get_record",
        &json!({ "path": r"Libs\Foundry\Records\Items\Boxes.XML" }),
    )?;
    assert_eq!(result["total"], 2, "two Records share it");
    let names: Vec<&str> = result["records"].as_array().map_or_default(|records| {
        records
            .iter()
            .filter_map(|record| record["$path"].as_str())
            .collect()
    });
    assert_eq!(
        names, ["libs/foundry/records/items/boxes"; 2],
        "case, separator and .xml do not matter"
    );
    Ok(())
}

#[test]
fn records_larger_than_the_cap_page_one_by_one_and_one_alone_is_an_error() -> TestResult {
    let install = install(dataforge(2, 150 << 10))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let path = json!({ "path": "libs/foundry/records/notes/long" });
    let (records, pages) = every_page(&mut server, "get_record", &path, "records")?;
    assert_eq!((records.len(), pages), (2, 2), "one Record per page");
    drop(server);
    let install = install_one_huge()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let message = server.error("get_record", &path)?;
    assert!(message.contains("response cap"), "too large: {message}");
    Ok(())
}

fn install_one_huge() -> TestResult<crate::support::Install> {
    install(dataforge(1, 300 << 10))
}

#[test]
fn record_lookups_that_find_nothing_are_tool_errors() -> TestResult {
    let install = install(dataforge(0, 0))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    for (arguments, expected) in [
        (json!({}), "either"),
        (json!({ "guid": AVENGER_TEXT, "path": "x" }), "either"),
        (json!({ "guid": "not-a-guid" }), "no GUID"),
        (
            json!({ "guid": "00000000-0000-0000-0000-000000000099" }),
            "search_records",
        ),
        (json!({ "path": "libs/missing" }), "search_records"),
        (json!({ "guid": "x".repeat(100) }), "longer than"),
    ] {
        let message = server.error("get_record", &arguments)?;
        assert!(message.contains(expected), "{arguments}: {message}");
    }
    Ok(())
}

#[test]
fn record_search_filters_by_record_path_and_type_in_record_order() -> TestResult {
    let install = install(dataforge(0, 0))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let ships = server.ok("search_records", &json!({ "path": "*/SPACESHIPS/*" }))?;
    assert_eq!(
        ships["records"][0],
        json!({
            "guid": AVENGER_TEXT,
            "name": "Ship.Avenger",
            "type": "Ship",
            "path": "libs/foundry/records/entities/spaceships/aegs_avenger"
        }),
        "the Record shape"
    );
    assert_eq!(
        texts(&ships["records"], "name"),
        ["Ship.Avenger", "Ship.Gladius"],
        "a glob over the record path"
    );
    let items = server.ok("search_records", &json!({ "type": "item" }))?;
    assert_eq!(
        texts(&items["records"], "name"),
        ["Item.Box", "Item.Crate"],
        "the type ignoring case"
    );
    let both = server.ok(
        "search_records",
        &json!({ "path": "gladius", "type": "Ship" }),
    )?;
    assert_eq!(
        texts(&both["records"], "name"),
        ["Ship.Gladius"],
        "both filters"
    );
    let all = server.ok("search_records", &json!({}))?;
    assert_eq!(all["total"], 5, "every Record without filters");
    Ok(())
}

#[test]
fn record_search_pages_with_a_cursor_bound_to_its_filters() -> TestResult {
    let install = install(many_items(25))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let arguments = json!({ "path": "items", "limit": 10 });
    let (records, pages) = every_page(&mut server, "search_records", &arguments, "records")?;
    let names: Vec<String> = records
        .iter()
        .filter_map(|record| record["name"].as_str().map(str::to_owned))
        .collect();
    let expected: Vec<String> = (0..25).map(|index| format!("Item.I{index:04}")).collect();
    assert_eq!((names, pages), (expected, 3), "every Record once, in order");
    let first = server.ok("search_records", &arguments)?;
    let cursor = first["next_cursor"].as_str().unwrap_or_default();
    let message = server.error(
        "search_records",
        &json!({ "path": "item_", "limit": 10, "cursor": cursor }),
    )?;
    assert!(message.contains("cursor"), "another filter: {message}");
    Ok(())
}

#[test]
fn an_archive_without_dataforge_says_so() -> TestResult {
    let install = crate::support::Install::standard()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let message = server.error("search_records", &json!({}))?;
    assert!(message.contains("DataForge"), "no DataForge: {message}");
    Ok(())
}

#[test]
fn record_keys_keep_the_record_xml_order() -> TestResult {
    let install = install(dataforge(0, 0))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let text = server.ok_text("get_record", &json!({ "guid": AVENGER_TEXT }))?;
    let records: Vec<Box<serde_json::value::RawValue>> =
        serde_json::from_str(&member(&text, "records")?)?;
    let record = records.first().ok_or("a record")?.get();
    assert_eq!(
        key_order(record)?,
        [
            "$type", "$guid", "$path", "$name", "mass", "size", "crew", "maker", "tags"
        ],
        "the fields in definition order, not sorted"
    );
    assert_eq!(
        key_order(&member(record, "maker")?)?,
        ["guid", "path", "type"],
        "a Reference link"
    );
    Ok(())
}
