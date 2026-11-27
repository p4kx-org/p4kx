// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{Identity, MediaRecord};
use serde_json::{Value, json};

use super::{DEFINITION, GRAPH_ENTRY, row};
use crate::support::audio_provider::{Origins, TestResult};

pub(super) fn check(rows: &[MediaRecord], origins: &Origins) -> TestResult {
    for (node, role, ancestors) in [
        (9, "input-link", [0, 1, 2, 5, 6, 7, 8]),
        (13, "output-link", [0, 1, 2, 5, 6, 7, 12]),
    ] {
        let (ordinal, entry) = definition(rows, node, role)?;
        let observed = contributions(origins, ordinal)?;
        let node_table = "cig-audio-cryxml-node";
        let mut expected = vec![
            source_use("payload", "payload", node_table, node, "element"),
            source_use(
                "archiveDirectoryMetadata",
                "selector",
                "archive-entry",
                entry,
                "path",
            ),
        ];
        expected.extend(
            ancestors.map(|index| source_use("payload", "selector", node_table, index, "element")),
        );
        assert_eq!(
            observed.len(),
            expected.len(),
            "exact role contributor count"
        );
        for contribution in expected {
            assert!(
                observed.contains(&contribution),
                "missing original role contribution {contribution}"
            );
        }
    }
    Ok(())
}

fn definition(rows: &[MediaRecord], node: u64, role: &str) -> TestResult<(usize, u64)> {
    let (_, element) = row(rows, "audio-control-element", &[node])?;
    assert_eq!(
        element
            .data
            .pointer("/payload/qualified_name")
            .and_then(Value::as_str),
        Some("0"),
        "{role} tag"
    );
    let (ordinal, definition) = row(rows, DEFINITION, &[node])?;
    assert_eq!(
        definition
            .data
            .pointer("/payload/role")
            .and_then(Value::as_str),
        Some(role),
        "authored parent selects the port role"
    );
    let Identity::Entry { entry, .. } = &definition.source.identity else {
        return Err("the CIG role must retain its physical Entry".into());
    };
    Ok((ordinal, entry.entry_index as u64))
}

fn contributions(origins: &Origins, ordinal: usize) -> TestResult<Vec<Value>> {
    let read_path = |path| origins.field_in("records", Some("audio-control"), ordinal, path, true);
    let field = read_path("/data/payload/role")?;
    let payload = read_path("/data/payload")?;
    assert_eq!(
        payload.get("sourceUsesRefs"),
        field.get("sourceUsesRefs"),
        "role and its aggregate cite the same actual selectors"
    );
    let references = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("role source uses")?;
    let mut observed = Vec::new();
    for reference in references {
        let proof = origins.physical_use(reference)?;
        assert_eq!(
            proof.source.pointer("/path/value"),
            Some(&json!(GRAPH_ENTRY)),
            "same original Entry"
        );
        observed.push(json!({
            "scope": proof.observation.get("scope"),
            "role": proof.observation.get("role"),
            "location": proof.observation.get("location")
        }));
    }
    Ok(observed)
}

fn source_use(scope: &str, role: &str, namespace: &str, index: u64, field: &str) -> Value {
    json!({
        "scope": scope,
        "role": role,
        "location": {
            "type": "subresource", "namespace": namespace,
            "indices": [index.to_string()], "field": field
        }
    })
}
