// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_testkit::{TestChunkFile, TestCrchModel, TestEntry, TestModel, TestNode};
use serde_json::{Value, json};

use crate::model_item_origin_checks::{
    assert_all_addresses, assert_site, field, inventory_uses, marker, uses,
};
use crate::support::{Install, TestResult};

const IVO: &str = "Data/ivo-nodes.cgf";
const CRCH: &str = "Data/crch-nodes.cgf";
const NMC: u32 = 0x7069_7FDA;
const RESPONSE_LIMIT: usize = 256 * 1024;

#[test]
fn ivo_nodes_bind_the_selected_original_matrix_scale_and_repeated_properties() -> TestResult {
    let authored = ivo_nodes();
    let property = |text: &[u8]| {
        authored
            .windows(text.len())
            .position(|part| part == text)
            .map(|at| at as u64)
            .ok_or("original property")
    };
    let first = property(b" tag = left \r\n")?;
    let second = property(b"tag=right\r\n")?;
    let empty = property(b"empty=\r\n")?;
    let file = TestChunkFile::ivo()
        .chunk(0x1111, 0, [0])
        .chunk(NMC, 0, authored)
        .chunk(NMC, 0, [0]);
    let start = *file.offsets().get(1).ok_or("selected node chunk")?;
    let install = Install::new(vec![
        TestEntry::zstd(IVO, file.bytes()).encrypted(),
        TestEntry::stored("data/IVO-NODES.CGF", "losing entry"),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call(
        "list_model_items",
        &json!({"path": IVO, "section": "nodes", "limit": 1}),
    )?;
    let fallback = &response["structuredContent"];
    assert_ivo_defaults(fallback, start)?;
    assert_all_addresses(&response)?;

    let response = server.call(
        "list_model_items",
        &json!({
            "path": IVO, "section": "nodes", "limit": 1, "cursor": fallback["next_cursor"],
        }),
    )?;
    let reply = &response["structuredContent"];
    assert_eq!(reply["items"][0]["index"], 1);
    assert_eq!(reply["items"][0]["parent"], 0);
    assert_eq!(reply["items"][0]["scale"], json!([2.0, 3.0, 4.0]));
    assert_eq!(reply["items"][0]["helper"], true);
    assert_eq!(
        reply["items"][0]["properties"],
        json!([
            {"key":"tag","value":"left"}, {"key":"tag","value":"right"}, {"key":"empty","value":""},
        ])
    );
    assert_ivo_spans(reply, start)?;
    assert_property_spans(reply, start + first, start + second, start + empty)?;
    assert_eq!(
        reply["provenance"]["sources"].as_array().map(Vec::len),
        Some(1)
    );
    assert_only_rows(reply, 1..2)?;
    assert_all_addresses(&response)
}

fn assert_ivo_defaults(fallback: &Value, start: u64) -> TestResult {
    assert_eq!(
        fallback.pointer("/items/0/name"),
        Some(&json!("root")),
        "first authored node"
    );
    assert_eq!(
        fallback.pointer("/items/0/transform/0/0"),
        Some(&json!(1.0)),
        "identity matrix default"
    );
    assert_eq!(
        fallback.pointer("/items/0/scale"),
        Some(&json!([1.0, 1.0, 1.0])),
        "unit scale default"
    );
    marker(field(fallback, "items/0/transform/0/0")?, true);
    marker(field(fallback, "model")?, false);
    assert_node_span(
        fallback,
        "items/0/transform/0/0",
        "matrixDefault",
        1,
        0,
        start + 112..start + 160,
    )?;
    assert_no_use(fallback, "items/0/transform/0/0", "matrixWord")?;
    assert_no_use(fallback, "items/0/scale/0", "scaleWord")?;
    Ok(())
}

#[test]
fn crch_nodes_use_the_actual_parent_id_and_transposed_original_word() -> TestResult {
    let file = TestCrchModel::cube().with_chunk(0x100B, 0x823, [1]).file();
    let start = *file.offsets().get(5).ok_or("child node offset")?;
    let mut bytes = file.bytes();
    let at = usize::try_from(start + 132)?;
    bytes
        .get_mut(at..at + 4)
        .ok_or("original translation word")?
        .copy_from_slice(&9.25_f32.to_le_bytes());
    let install = Install::new(vec![
        TestEntry::stored(CRCH, bytes),
        TestEntry::stored("data/CRCH-NODES.CGF", "losing entry").encrypted(),
    ])?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    let reply = server.ok(
        "list_model_items",
        &json!({"path": CRCH, "section": "nodes"}),
    )?;
    assert_eq!(reply["total"], 2);
    assert_eq!(reply["items"][1]["parent"], 0);
    assert_eq!(reply["items"][1]["transform"][0][3], 9.25);
    assert!(reply["items"][1]["geometry_type"].is_null());
    marker(field(&reply, "items/1/parent")?, false);
    assert_site(
        &reply,
        "items/1/parent",
        "cryEngineChunkTable",
        "id",
        &["2", "52", "56"],
    )?;
    assert_site(
        &reply,
        "total",
        "cryEngineChunkTable",
        "nodeTypeAndVersionWords",
        &["16", "20", "16", "8"],
    )?;
    assert_node_span(
        &reply,
        "items/1/parent",
        "parent",
        5,
        1,
        start + 68..start + 72,
    )?;
    assert_node_span(
        &reply,
        "items/1/transform/0/3",
        "matrixWord",
        5,
        1,
        start + 132..start + 136,
    )?;
    assert_eq!(
        reply["provenance"]["sources"].as_array().map(Vec::len),
        Some(1)
    );
    Ok(())
}

#[test]
fn a_synthesized_root_uses_the_actual_empty_node_count_and_model_path() -> TestResult {
    let file =
        TestChunkFile::ivo().chunk(NMC, 0, TestModel::cube().with_nodes(Vec::new()).nmc_chunk());
    let start = *file.offsets().first().ok_or("empty node chunk start")?;
    let install = Install::new(vec![TestEntry::stored(IVO, file.bytes()).encrypted()])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call(
        "list_model_items",
        &json!({"path": IVO, "section": "nodes"}),
    )?;
    let reply = &response["structuredContent"];
    assert_eq!(reply["total"], 1);
    assert_eq!(reply["items"][0]["name"], "ivo-nodes");
    marker(field(reply, "items/0/name")?, true);
    assert_site(
        reply,
        "items/0/name",
        "cryEngineNodes",
        "nodeCount",
        &["0", &(start + 4).to_string(), &(start + 8).to_string()],
    )?;
    assert!(
        uses(reply, "items/0/name")?.iter().any(|site| site
            .pointer("/location/type")
            .and_then(Value::as_str)
            == Some("metadata")
            && site.pointer("/location/field").and_then(Value::as_str) == Some("path")),
        "the generated root name uses the original Entry path"
    );
    assert_no_use(reply, "items/0/name", "name")?;
    assert_no_use(reply, "items/0/transform/0/0", "matrixWord")?;
    assert_all_addresses(&response)
}

#[test]
fn node_pages_charge_the_origin_metadata_and_preserve_every_row() -> TestResult {
    let count = 180;
    let nodes = (0..count)
        .map(|index| TestNode {
            name: format!("node {index}"),
            parent: None,
            kind: 0,
            local: [
                [1.0, 0.0, 0.0, 5.0],
                [0.0, 1.0, 0.0, 6.0],
                [0.0, 0.0, 1.0, 7.0],
            ],
        })
        .collect();
    let file = TestChunkFile::ivo().chunk(NMC, 0, TestModel::cube().with_nodes(nodes).nmc_chunk());
    let install = Install::new(vec![TestEntry::stored(IVO, file.bytes())])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut args = json!({"path": IVO, "section": "nodes", "limit": 1000});
    let mut next = 0;
    loop {
        let response = server.call("list_model_items", &args)?;
        let reply = &response["structuredContent"];
        assert_eq!(reply["total"], count);
        assert!(serde_json::to_vec(reply)?.len() <= RESPONSE_LIMIT);
        let rows = reply["items"].as_array().ok_or("node rows")?;
        let first = next;
        assert!(!rows.is_empty(), "the page advances");
        for row in rows {
            assert_eq!(row["index"], next);
            next += 1;
        }
        assert_only_rows(reply, first..next)?;
        assert_all_addresses(&response)?;
        if let Some(cursor) = reply.get("next_cursor") {
            args["cursor"] = cursor.clone();
        } else {
            assert!(first > 0, "origin metadata forces pagination");
            break;
        }
    }
    assert_eq!(next, count);
    Ok(())
}

fn ivo_nodes() -> Vec<u8> {
    TestModel::cube()
        .with_nodes(vec![
            TestNode {
                name: "root\nignored".to_owned(),
                parent: None,
                local: [[0.0; 4]; 3],
                kind: 0,
            },
            TestNode {
                name: "branch".to_owned(),
                parent: Some(0),
                kind: 2,
                local: [
                    [1.0, 0.0, 0.0, 5.0],
                    [0.0, 1.0, 0.0, 6.0],
                    [0.0, 0.0, 1.0, 7.0],
                ],
            },
        ])
        .with_node_details(
            1,
            [2.0, 3.0, 4.0],
            &[
                ("", "skipped"),
                (" tag ", " left "),
                ("tag", "right"),
                ("empty", ""),
            ],
        )
        .nmc_chunk()
}

fn assert_ivo_spans(reply: &Value, start: u64) -> TestResult {
    assert_site(
        reply,
        "total",
        "cryEngineChunkTable",
        "nodeMeshTypePrefix",
        &["16", "20", "16", "2"],
    )?;
    for (output, source, span) in [
        ("transform/0/3", "matrixWord", 332..336),
        ("scale/2", "scaleWord", 376..380),
        ("parent", "parent", 388..390),
        ("geometry_type", "geometryType", 390..392),
        ("helper", "geometryType", 390..392),
    ] {
        assert_node_span(
            reply,
            &format!("items/0/{output}"),
            source,
            1,
            1,
            start + span.start..start + span.end,
        )?;
    }
    Ok(())
}

fn assert_property_spans(reply: &Value, first: u64, second: u64, empty: u64) -> TestResult {
    for (output, source, span) in [
        ("0/key", "key", first + 1..first + 4),
        ("0/value", "value", first + 7..first + 11),
        ("1/key", "key", second..second + 3),
        ("1/value", "value", second + 4..second + 9),
        ("2/value", "propertyLine", empty..empty + 7),
    ] {
        assert_node_span(
            reply,
            &format!("items/0/properties/{output}"),
            source,
            1,
            1,
            span,
        )?;
    }
    assert_no_use(reply, "items/0/properties/2/value", "value")
}

fn assert_only_rows(reply: &Value, expected: Range<usize>) -> TestResult {
    for site in inventory_uses(reply)? {
        if site.pointer("/location/namespace").and_then(Value::as_str) != Some("cryEngineNodes")
            || site.pointer("/location/field").and_then(Value::as_str) == Some("nodeCount")
        {
            continue;
        }
        let ordinal = site
            .pointer("/location/indices/1")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<usize>().ok());
        assert!(
            ordinal.is_some_and(|ordinal| expected.contains(&ordinal)),
            "only returned node ordinals"
        );
    }
    Ok(())
}

fn assert_no_use(reply: &Value, output: &str, field: &str) -> TestResult {
    assert!(
        uses(reply, output)?
            .iter()
            .all(|site| site.pointer("/location/field").and_then(Value::as_str) != Some(field)),
        "{output} must omit the unconsumed {field} word"
    );
    Ok(())
}

fn assert_node_span(
    reply: &Value,
    output: &str,
    name: &str,
    chunk: usize,
    ordinal: usize,
    span: Range<u64>,
) -> TestResult {
    assert_site(
        reply,
        output,
        "cryEngineNodes",
        name,
        &[
            &chunk.to_string(),
            &ordinal.to_string(),
            &span.start.to_string(),
            &span.end.to_string(),
        ],
    )
}
