// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::check_glb;
use serde_json::{Value, json};

use super::support::TestResult;

pub(super) struct ExpectedSubmesh<'a> {
    pub(super) source: &'a str,
    pub(super) encrypted: bool,
    pub(super) start: u64,
    pub(super) ordinal: usize,
    pub(super) node: u16,
    pub(super) material: u16,
}

pub(super) fn assert_draws(proof: &[Value], bytes: &[u8]) -> TestResult {
    let emitted = check_glb(bytes)?;
    let mut rows = proof.iter().filter(|row| is_primitive(row));
    let mut count = 0_u64;
    for mesh in emitted.document.meshes() {
        for draw in mesh.primitives() {
            let row = rows.next().ok_or("every emitted draw has a row")?;
            for (key, value) in [
                ("mesh", json!(mesh.index())),
                ("primitive", json!(draw.index())),
                (
                    "indexAccessor",
                    json!(draw.indices().map(|index| index.index())),
                ),
                ("materialIndex", json!(draw.material().index())),
            ] {
                assert_eq!(row.get(key), Some(&value), "actual emitted {key}");
            }
            count += 1;
        }
    }
    assert!(rows.next().is_none(), "no row without a GLB primitive");
    assert_eq!(
        proof.first().and_then(|row| row.get("version")),
        Some(&json!(2)),
        "emitted primitive map version"
    );
    assert_eq!(
        proof.last().and_then(|row| row.get("primitives")),
        Some(&json!(count.to_string())),
        "all emitted primitives reconciled"
    );
    assert_group_order(proof);
    Ok(())
}

fn assert_group_order(proof: &[Value]) {
    let mut mesh = None;
    let mut in_vertices = false;
    for row in proof {
        match row.get("type").and_then(Value::as_str) {
            Some("mesh") => {
                mesh = row.get("mesh");
                in_vertices = false;
            }
            Some("primitive") => {
                assert!(!in_vertices, "primitive rows precede vertices");
                assert_eq!(mesh, row.get("mesh"), "same emitted mesh");
            }
            Some("vertex") => in_vertices = true,
            _ => {}
        }
    }
}

pub(super) fn assert_submesh(
    proof: &[Value],
    row: &Value,
    expected: &ExpectedSubmesh<'_>,
) -> TestResult {
    for (key, field, value, range, role) in [
        (
            "row",
            "submeshRow",
            json!(expected.ordinal),
            (expected.start, expected.start + 48),
            "selector",
        ),
        (
            "node",
            "submeshNode",
            json!(expected.node),
            (expected.start + 2, expected.start + 4),
            "payload",
        ),
        (
            "material",
            "submeshMaterial",
            json!(expected.material),
            (expected.start, expected.start + 2),
            "payload",
        ),
    ] {
        let fact = row
            .get("sourceSubmesh")
            .and_then(|row| row.get(key))
            .ok_or("raw submesh value")?;
        assert_eq!(fact.get("value"), Some(&value), "{key} raw source value");
        assert_eq!(fact.get("state"), Some(&json!("present")), "{key} state");
        assert_eq!(
            fact.get("containsEncryptedSource"),
            Some(&json!(expected.encrypted)),
            "{key} encryption"
        );
        assert_eq!(
            fact.get("sourceSetComplete"),
            Some(&json!(true)),
            "{key} completeness"
        );
        assert_geometry_use(proof, fact, field, role, range, expected.source)?;
    }
    Ok(())
}

fn assert_geometry_use(
    proof: &[Value],
    fact: &Value,
    field: &str,
    role: &str,
    range: (u64, u64),
    path: &str,
) -> TestResult {
    let refs = fact
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source use list")?;
    assert_eq!(refs.len(), 1, "one exact geometry word per field");
    let usage = proof
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .find(|usage| usage.get("id") == refs.first())
        .ok_or("actual source use")?;
    assert_eq!(usage.get("role"), Some(&json!(role)), "{field} use role");
    assert_eq!(
        usage.pointer("/location/field"),
        Some(&json!(field)),
        "{field} use field"
    );
    let bounds = [range.0.to_string(), range.1.to_string()];
    assert_eq!(
        usage.pointer("/location/indices"),
        Some(&json!(bounds)),
        "{field} bounds"
    );
    let original = proof
        .iter()
        .filter_map(|row| row.get("source"))
        .find(|source| source.get("id") == usage.get("source"))
        .ok_or("physical source")?;
    assert_eq!(
        original.pointer("/path/value"),
        Some(&json!(path)),
        "{field} physical Archive path"
    );
    Ok(())
}

pub(super) fn primitive_row(
    proof: &[Value],
    mesh: usize,
    ordinal: usize,
) -> Result<&Value, Box<dyn std::error::Error>> {
    proof
        .iter()
        .filter(|row| is_primitive(row))
        .find(|row| {
            row.get("mesh") == Some(&json!(mesh)) && row.get("primitive") == Some(&json!(ordinal))
        })
        .ok_or_else(|| format!("missing emitted mesh {mesh} primitive {ordinal}").into())
}

fn is_primitive(row: &Value) -> bool {
    row.get("type").and_then(Value::as_str) == Some("primitive")
}
