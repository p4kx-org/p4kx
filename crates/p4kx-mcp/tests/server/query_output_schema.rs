// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::forge;
use crate::support::{Server, TestResult};

const QUERY: &str = "from FixtureQuery select value";
const HEALTHY_PATH: &str = "fixture/query/healthy";
const FAILED_PATH: &str = "fixture/query/unreadable";

#[test]
fn query_pages_validate_against_the_advertised_output_schema() -> TestResult {
    let install = forge::install(database()?)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let schema = output_schema(&mut server)?;
    let validator = jsonschema::validator_for(&schema)?;

    let first = server.ok("run_query", &json!({"query": QUERY, "limit": 1}))?;
    validator
        .validate(&first)
        .map_err(|error| error.to_string())?;
    assert_eq!(
        first.get("failed"),
        Some(&json!([])),
        "successful page carries zero failures"
    );
    assert_eq!(
        first.get("rows"),
        Some(&json!([{"record path": HEALTHY_PATH, "value": 7}])),
        "readable physical row returns data"
    );

    assert_empty_failure_origin(&first)?;
    let cursor = first
        .get("next_cursor")
        .and_then(Value::as_str)
        .ok_or("cursor for the unreadable Record")?;
    let last = server.ok(
        "run_query",
        &json!({"query": QUERY, "limit": 1, "cursor": cursor}),
    )?;
    validator
        .validate(&last)
        .map_err(|error| error.to_string())?;
    assert_failed_record(&last)?;
    let empty = server.ok(
        "run_query",
        &json!({"query":"at \"fixture/query/no-records\""}),
    )?;
    validator
        .validate(&empty)
        .map_err(|error| error.to_string())?;
    assert_eq!(
        empty.get("failed"),
        Some(&json!([])),
        "empty selection retains the required array"
    );
    assert_empty_failure_origin(&empty)?;
    invalid_query(&mut server)
}

fn assert_failed_record(last: &Value) -> TestResult {
    assert_eq!(
        last.get("total"),
        Some(&json!(2)),
        "the failed Record counts as a selected occurrence"
    );
    assert_eq!(
        last.get("rows"),
        Some(&json!([])),
        "an unreadable Record has a failure instead of a successful row"
    );
    let Some([failed]) = last
        .get("failed")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return Err("one explicitly represented failed row".into());
    };
    assert_eq!(
        failed.get("record_path"),
        Some(&json!(FAILED_PATH)),
        "the failed Record retains its path"
    );
    assert!(
        failed
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains("instance")),
        "the unreadable instance is diagnosed"
    );
    let uses = crate::model_item_origin_checks::uses(last, "failed")?;
    assert!(
        uses.iter()
            .any(|used| used.pointer("/location/field") == Some(&json!("fileName"))),
        "failed array retains the actual Record path contribution"
    );
    Ok(())
}

fn assert_empty_failure_origin(page: &Value) -> TestResult {
    let origin = crate::model_item_origin_checks::field(page, "failed")?;
    assert_eq!(
        origin.get("sourceUsesRefs"),
        Some(&json!([])),
        "an empty failure list contains no failed Record payload"
    );
    Ok(())
}

fn invalid_query(server: &mut Server) -> TestResult {
    let response = server.call("run_query", &json!({"query": "from"}))?;
    assert_eq!(
        response.get("isError"),
        Some(&json!(true)),
        "invalid syntax is still an RPC tool error"
    );
    assert!(
        response.get("structuredContent").is_none(),
        "a parse error carries only the error response"
    );
    let text = response
        .pointer("/content/0/text")
        .and_then(Value::as_str)
        .ok_or("parse error text")?;
    assert!(
        text.contains("at bytes 4..4"),
        "parse error retains the original byte span: {text}"
    );
    Ok(())
}

fn output_schema(server: &mut Server) -> TestResult<Value> {
    let tool = server
        .tools()?
        .into_iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some("run_query"))
        .ok_or("run_query in tools/list")?;
    tool.get("outputSchema")
        .cloned()
        .ok_or_else(|| "advertised run_query output schema".into())
}

fn database() -> TestResult<Vec<u8>> {
    let mut bytes = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("FixtureQuery").field("value", TestType::Int32))
        .record(
            TestRecord::new(
                1,
                "FixtureQuery",
                "FixtureQuery.Healthy",
                format!("{HEALTHY_PATH}.xml"),
            )
            .value("value", TestValue::Int32(7)),
        )
        .record(
            TestRecord::new(
                2,
                "FixtureQuery",
                "FixtureQuery.Unreadable",
                format!("{FAILED_PATH}.xml"),
            )
            .value("value", TestValue::Int32(9)),
        )
        .build();
    invalidate_second_instance(&mut bytes)?;
    Ok(bytes)
}

fn invalidate_second_instance(bytes: &mut [u8]) -> TestResult {
    const HEADER_BYTES: usize = 120;
    const V8_RECORD_BYTES: usize = 36;
    const V8_INSTANCE_OFFSET: usize = 32;
    const PRECEDING_TABLE_ROW_BYTES: [usize; 4] = [16, 12, 8, 8];
    let (words, _) = bytes.as_chunks::<4>();
    let counts = words.get(4..8).ok_or("original table counts")?;
    let records = counts
        .iter()
        .zip(PRECEDING_TABLE_ROW_BYTES)
        .try_fold(HEADER_BYTES, |offset, (count, size)| {
            usize::try_from(u32::from_le_bytes(*count))
                .ok()?
                .checked_mul(size)?
                .checked_add(offset)
        })
        .ok_or("original Record table offset")?;
    let start = records
        .checked_add(V8_RECORD_BYTES + V8_INSTANCE_OFFSET)
        .ok_or("instance offset")?;
    let end = start
        .checked_add(size_of::<u16>())
        .ok_or("instance bound")?;
    bytes
        .get_mut(start..end)
        .ok_or("original instance index bytes")?
        .copy_from_slice(&u16::MAX.to_le_bytes());
    Ok(())
}
