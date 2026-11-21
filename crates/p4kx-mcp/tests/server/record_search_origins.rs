// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestEntry, TestRecord, TestStruct};
use serde_json::{Value, json};

use crate::forge::{AVENGER_TEXT, dataforge, install, many_items};
use crate::model_item_origin_checks::{assert_all_addresses, assert_site, field, marker, uses};
use crate::support::{Install, TestResult};

#[test]
fn a_search_page_binds_its_record_fields_to_the_cached_encrypted_dataforge() -> TestResult {
    let install = Install::new(vec![
        TestEntry::zstd("Data/Game2.dcb", dataforge(0, 0)).encrypted(),
        TestEntry::zstd("Data/Game.dcb", many_items(1)),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let query = json!({"path": "spaceships", "type": "Ship", "limit": 1});
    let response = server.call("search_records", &query)?;
    let reply = response.get("structuredContent").ok_or("search response")?;
    assert_eq!(reply.get("total"), Some(&json!(2)), "legacy filtered count");
    assert_eq!(
        reply.pointer("/records/0/guid"),
        Some(&json!(AVENGER_TEXT)),
        "legacy first matching record"
    );
    assert_ship_search_evidence(reply)?;
    assert_all_addresses(&response)?;
    let mut query = query;
    query["cursor"] = reply.get("next_cursor").ok_or("next page")?.clone();
    let next = server.ok("search_records", &query)?;
    assert_eq!(
        next.pointer("/records/0/name"),
        Some(&json!("Ship.Gladius")),
        "the cached page advances"
    );
    marker(field(&next, "records/0/name")?, true);
    Ok(())
}

fn assert_ship_search_evidence(reply: &Value) -> TestResult {
    for (output, original) in [
        ("guid", "guid"),
        ("name", "recordName"),
        ("path", "fileName"),
        ("type", "sourceType"),
    ] {
        let path = format!("records/0/{output}");
        marker(field(reply, &path)?, true);
        assert!(
            uses(reply, &path)?.iter().any(|site| {
                site.pointer("/location/type") == Some(&json!("dataForge"))
                    && site.pointer("/location/recordIndex") == Some(&json!(1))
                    && site.pointer("/location/field") == Some(&json!(original))
            }),
            "physical Record 1's original {original}"
        );
    }
    marker(field(reply, "total")?, true);
    marker(field(reply, "next_cursor")?, true);

    assert_site(
        reply,
        "records/0/type",
        "dataforgeStructDefinition",
        "name",
        &["2"],
    )?;
    assert_ship_navigation(reply)?;
    assert_eq!(
        reply
            .pointer("/provenance/sources")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "only the worker's chosen DataForge is in the inventory"
    );
    Ok(())
}

fn assert_ship_navigation(reply: &Value) -> TestResult {
    assert_site(
        reply,
        "total",
        "dataforgeRecordSurvey",
        "fileNameOffsets",
        &["0", "5"],
    )?;
    assert_site(
        reply,
        "total",
        "dataforgeRecordSurvey",
        "testedRecordTypes",
        &["1", "3"],
    )?;
    assert_site(
        reply,
        "total",
        "dataforgeRecordSurvey",
        "testedTypeNames",
        &["2", "3"],
    )?;
    assert_site(
        reply,
        "next_cursor",
        "dataforgeRecordSurvey",
        "orderNameOffsets",
        &["0", "5"],
    )?;
    Ok(())
}

#[test]
fn equivalent_archive_and_standalone_records_keep_their_own_provenance() -> TestResult {
    let bytes = dataforge(0, 0);
    let install = install(bytes.clone())?;
    let path = install.archive().with_file_name("synthetic.dcb");
    std::fs::write(&path, bytes)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let embedded = server.ok("search_records", &json!({"path": "avenger"}))?;
    let external = server.ok(
        "search_records",
        &json!({"archive": path, "path": "avenger"}),
    )?;
    assert_eq!(
        external.get("records"),
        embedded.get("records"),
        "identical Record values"
    );
    for output in ["records/0/guid", "records/0/type", "total"] {
        marker(field(&embedded, output)?, false);
        let unknown = field(&external, output)?;
        assert_eq!(
            unknown.get("containsEncryptedSource"),
            Some(&Value::Null),
            "the standalone file has unknown P4K lineage"
        );
        assert_eq!(
            unknown.get("sourceSetComplete"),
            Some(&json!(true)),
            "the standalone read has its own complete evidence"
        );
    }
    assert_eq!(
        external.pointer("/provenance/sources/0/kind"),
        Some(&json!("file")),
        "the source is the actual standalone input"
    );
    let tools = server.tools()?;
    let search = tools
        .iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some("search_records"))
        .ok_or("search tool")?;
    let required = search
        .pointer("/outputSchema/required")
        .and_then(Value::as_array)
        .ok_or("required output fields")?;
    assert!(
        required.contains(&json!("fieldOrigins")) && required.contains(&json!("provenance")),
        "new writers must include both provenance fields"
    );
    Ok(())
}

#[test]
fn record_search_pages_charge_evidence_without_skipping_rows() -> TestResult {
    let count = 400;
    let install = install(many_items(count))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut query = json!({"path": "items", "limit": 1000});
    let mut seen = 0;
    let mut pages = 0;
    loop {
        let response = server.call("search_records", &query)?;
        let reply = response.get("structuredContent").ok_or("search page")?;
        assert_eq!(reply.get("total"), Some(&json!(count)), "unchanged total");
        assert!(
            serde_json::to_vec(reply)?.len() <= 256 * 1024,
            "the whole structured response fits"
        );
        assert_all_addresses(&response)?;
        let records = reply
            .get("records")
            .and_then(Value::as_array)
            .ok_or("records")?;
        assert!(!records.is_empty(), "each page advances");
        for (index, record) in records.iter().enumerate() {
            assert_eq!(
                record.get("name"),
                Some(&json!(format!("Item.I{seen:04}"))),
                "complete original search order"
            );
            marker(field(reply, &format!("records/{index}/name"))?, false);
            seen += 1;
        }
        pages += 1;
        let Some(cursor) = reply.get("next_cursor") else {
            break;
        };
        query["cursor"] = cursor.clone();
    }
    assert!(pages > 1, "the provenance takes space within the budget");
    assert_eq!(seen, count, "every matching Record appears once");
    Ok(())
}

#[test]
fn an_oversized_filter_survey_keeps_its_known_encrypted_contribution() -> TestResult {
    let count = 50_000_u32;
    let bytes = (0..count)
        .fold(
            DataForgeBuilder::new().structure(TestStruct::new("Item")),
            |builder, index| {
                let folder = if index % 2 == 0 { "wanted" } else { "other" };
                builder.record(TestRecord::new(
                    u128::from(index) + 1,
                    "Item",
                    format!("Item.I{index:05}"),
                    format!("data/{folder}/record_{index:05}.xml"),
                ))
            },
        )
        .build();
    let install = Install::new(vec![TestEntry::zstd("Data/Game2.dcb", bytes).encrypted()])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let query = json!({"path": "wanted", "type": "Item", "limit": 1});
    let result = server.ok("search_records", &query)?;
    assert_eq!(
        result.get("total"),
        Some(&json!(count / 2)),
        "the full selection is still counted"
    );
    assert_eq!(
        result.pointer("/records/0/name"),
        Some(&json!("Item.I00000")),
        "the first matching row remains whole"
    );
    assert!(
        serde_json::to_vec(&result)?.len() <= 256 * 1024,
        "the whole response stays under the existing cap"
    );
    for output in ["total", "next_cursor"] {
        let origin = field(&result, output)?;
        assert_eq!(
            origin.get("sourceSetComplete"),
            Some(&json!(false)),
            "the full sparse survey cannot fit"
        );
        assert_eq!(
            origin.get("containsEncryptedSource"),
            Some(&json!(true)),
            "known encryption survives partial evidence"
        );
        assert_site(
            &result,
            output,
            "dataforgeRecordTable",
            "definitionCount",
            &[],
        )?;
    }
    marker(field(&result, "records/0/name")?, true);
    Ok(())
}
