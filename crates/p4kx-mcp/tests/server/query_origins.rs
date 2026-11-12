// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use crate::forge::{dataforge, install, many_items};
use crate::model_item_origin_checks::{assert_all_addresses, assert_site, field, marker, uses};
use crate::support::{Install, TestResult};

#[test]
fn query_tracks_actual_cells_and_short_circuited_selection() -> TestResult {
    let bytes = dataforge(0, 0);
    let install = Install::new(vec![
        TestEntry::zstd("Data/Game2.dcb", bytes.clone()).encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call(
        "run_query",
        &json!({"query": "from Ship select crew, maker->code, tags[*] where crew >= 2 or tags[*] = \"Gladius\""}),
    )?;
    let page = response.get("structuredContent").ok_or("query page")?;
    assert_eq!(
        page.get("total"),
        Some(&json!(2)),
        "each distinct predicate witness is selected"
    );
    assert_eq!(
        page.pointer("/rows/0/tags[*]/1"),
        Some(&json!("Avenger")),
        "unchanged selected data"
    );
    for output in [
        "rows/0/crew",
        "rows/0/maker->code",
        "rows/0/tags[*]/1",
        "total",
    ] {
        marker(field(page, output)?, true);
    }
    let predicate = original_bytes(page, "total", "text", &bytes)?;
    assert!(
        predicate.contains(&b"Gladius".as_slice()),
        "the second branch was read"
    );
    assert!(
        !predicate.contains(&b"Avenger".as_slice()),
        "the first row short-circuited"
    );
    let cell = original_bytes(page, "rows/0/tags[*]/1", "text", &bytes)?;
    assert_eq!(
        cell,
        [b"Avenger".as_slice()],
        "the returned array element uses its own value"
    );
    assert_site(
        page,
        "rows/0/maker->code",
        "dataforgeInstance",
        "selection",
        &["0", "0"],
    )?;
    assert_all_addresses(&response)
}

fn original_bytes<'source>(
    page: &Value,
    output: &str,
    label: &str,
    original: &'source [u8],
) -> TestResult<Vec<&'source [u8]>> {
    let mut matches = Vec::new();
    for site in uses(page, output)? {
        if site.pointer("/location/namespace") != Some(&json!("dataforgeDecodedBytes"))
            || site.pointer("/location/field") != Some(&json!(label))
        {
            continue;
        }
        let offset = |path: &str| -> TestResult<usize> {
            Ok(site
                .pointer(path)
                .and_then(Value::as_str)
                .ok_or("byte offset")?
                .parse()?)
        };
        matches.push(
            original
                .get(offset("/location/indices/0")?..offset("/location/indices/1")?)
                .ok_or("valid decoded-byte span")?,
        );
    }
    Ok(matches)
}

fn branching_fixture() -> Vec<u8> {
    let child = |kind, number, label| {
        TestValue::Struct(
            TestInstance::new(kind)
                .value("number", TestValue::Int32(number))
                .value("label", TestValue::text(label)),
        )
    };
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Base")
                .field("number", TestType::Int32)
                .field("label", TestType::String),
        )
        .structure(TestStruct::new("Alpha").parent("Base"))
        .structure(TestStruct::new("Beta").parent("Base"))
        .structure(
            TestStruct::new("Holder").array("nodes", TestType::StrongPointer("Base".to_owned())),
        )
        .record(
            TestRecord::new(1, "Holder", "Holder.One", "holder.xml").value(
                "nodes",
                TestValue::Array(vec![
                    child("Alpha", 17, "chosen"),
                    child("Beta", 99, "excluded"),
                ]),
            ),
        )
        .build()
}

#[test]
fn shared_array_prefix_keeps_only_the_admitted_payload_on_each_output_element() -> TestResult {
    let bytes = branching_fixture();
    let install = install(bytes.clone())?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call(
        "run_query",
        &json!({
            "query": "from Holder select nodes[Alpha].number, nodes[Alpha].label",
        }),
    )?;
    let page = response.get("structuredContent").ok_or("query result")?;
    assert_eq!(
        page.pointer("/rows/0/nodes[Alpha].number/0"),
        Some(&json!(17)),
        "same numeric branch"
    );
    assert_eq!(
        page.pointer("/rows/0/nodes[Alpha].label/0"),
        Some(&json!("chosen")),
        "same text branch"
    );
    for output in [
        "rows/0/nodes[Alpha].number/0",
        "rows/0/nodes[Alpha].label/0",
    ] {
        assert_site(page, output, "dataforgeInstance", "selection", &["1", "0"])?;
        let locations = uses(page, output)?;
        assert!(
            locations.iter().all(|use_site| {
                use_site.pointer("/location/namespace") != Some(&json!("dataforgeInstance"))
                    || use_site.pointer("/location/indices") != Some(&json!(["2", "0"]))
            }),
            "the Beta instance supplied no element payload"
        );
        marker(field(page, output)?, false);
    }
    let original = original_bytes(page, "rows/0/nodes[Alpha].label/0", "text", &bytes)?;
    assert_eq!(
        original,
        [b"chosen".as_slice()],
        "actual selected text span"
    );
    assert_all_addresses(&response)
}

#[test]
fn source_evidence_pages_without_skipping_original_rows() -> TestResult {
    let count = 440;
    let install = install(many_items(count))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let query = "from Item select mass";
    let mut arguments = json!({"query": query, "limit": 1000});
    let mut expected = 0;
    let mut pages = 0;
    loop {
        let response = server.call("run_query", &arguments)?;
        let page = response.get("structuredContent").ok_or("query page")?;
        assert_eq!(
            page.get("total"),
            Some(&json!(count)),
            "same total on every page"
        );
        assert!(
            serde_json::to_vec(page)?.len() <= 256 * 1024,
            "provenance fits with values"
        );
        assert_all_addresses(&response)?;
        let rows = page.get("rows").and_then(Value::as_array).ok_or("rows")?;
        assert!(!rows.is_empty(), "cursor progresses");
        for row in rows {
            assert_eq!(
                row.get("record path"),
                Some(&json!(format!(
                    "libs/foundry/records/items/item_{expected:04}"
                ))),
                "path order and continuous pages",
            );
            expected += 1;
        }
        pages += 1;
        let Some(cursor) = page.get("next_cursor") else {
            break;
        };
        arguments = json!({"query": query, "limit": 1000, "cursor": cursor});
    }
    assert_eq!(expected, count, "all original rows returned");
    assert!(pages > 1, "evidence made a single response too large");
    Ok(())
}

#[test]
fn large_where_survey_keeps_a_known_encrypted_witness_on_cached_pages() -> TestResult {
    let count = 3100;
    let install = Install::new(vec![
        TestEntry::zstd("Data/Game2.dcb", many_items(count)).encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let query = "from Item select mass where mass >= 0";
    let first = server.ok("run_query", &json!({"query": query, "limit": 1}))?;
    let cursor = first.get("next_cursor").ok_or("cursor")?;
    let second = server.ok(
        "run_query",
        &json!({"query": query, "limit": 1, "cursor": cursor}),
    )?;
    for page in [&first, &second] {
        assert_eq!(
            page.get("total"),
            Some(&json!(count)),
            "unchanged cached selection"
        );
        assert!(
            serde_json::to_vec(page)?.len() <= 256 * 1024,
            "the whole result fits"
        );
        let origin = field(page, "total")?;
        assert_eq!(
            origin.get("containsEncryptedSource"),
            Some(&json!(true)),
            "known source is encrypted"
        );
        assert_eq!(
            origin.get("sourceSetComplete"),
            Some(&json!(false)),
            "detailed survey exceeds response budget"
        );
        assert!(
            uses(page, "total")?.iter().any(|site| {
                site.pointer("/location/namespace") == Some(&json!("dataforgeDecodedBytes"))
            }),
            "an actual predicate span survives the compact response"
        );
    }
    Ok(())
}

#[test]
fn empty_follow_prefix_and_resolved_links_supply_the_actual_count_evidence() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Maker").field("code", TestType::String))
        .structure(
            TestStruct::new("Node")
                .field("number", TestType::Int32)
                .field("maker", TestType::ReferenceTo("Maker".to_owned())),
        )
        .structure(TestStruct::new("Holder").array("nodes", TestType::Class("Node".to_owned())))
        .record(TestRecord::new(1, "Holder", "Holder.One", "holder.xml"))
        .build();
    let install = Install::new(vec![TestEntry::zstd("Data/Game2.dcb", bytes).encrypted()])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let empty = server.ok(
        "run_query",
        &json!({
            "query": "from Holder select nodes[*].number, nodes[*].maker->code",
        }),
    )?;
    assert_eq!(
        empty.get("unresolved_references"),
        Some(&json!(0)),
        "no link was reached"
    );
    marker(field(&empty, "unresolved_references")?, true);
    assert!(
        uses(&empty, "unresolved_references")?
            .iter()
            .any(|site| { site.pointer("/location/field") == Some(&json!("arrayHeader")) }),
        "the encrypted empty array was consulted by the shared prefix"
    );
    let local = server.ok(
        "run_query",
        &json!({"query": "from Holder select nodes[*].number"}),
    )?;
    marker(field(&local, "unresolved_references")?, false);
    assert!(
        uses(&local, "unresolved_references")?.is_empty(),
        "there is no follow operation"
    );
    Ok(())
}

#[test]
fn a_resolved_follow_retains_its_target_in_the_zero_unresolved_count() -> TestResult {
    let install = Install::new(vec![
        TestEntry::zstd("Data/Game2.dcb", dataforge(0, 0)).encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let page = server.ok(
        "run_query",
        &json!({"query": "from Ship select maker->code, maker->code"}),
    )?;
    assert_eq!(
        page.get("unresolved_references"),
        Some(&json!(0)),
        "the shared target resolves"
    );
    assert_site(
        &page,
        "unresolved_references",
        "dataforgeInstance",
        "selection",
        &["0", "0"],
    )?;
    assert!(
        uses(&page, "unresolved_references")?
            .iter()
            .all(|site| { site.pointer("/location/field") != Some(&json!("text")) }),
        "the target's emitted scalar is irrelevant to resolving its link"
    );
    marker(field(&page, "unresolved_references")?, true);
    Ok(())
}
