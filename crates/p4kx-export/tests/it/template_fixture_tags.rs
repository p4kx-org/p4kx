// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Context;

pub(crate) fn path(graph: &dyn Context<'_>, node: &Value, expected: Option<&str>) -> TestResult {
    let reference = graph
        .owner()
        .reference(crate::flow_owner_graph::ordinal(node)?)?;
    let matching: Vec<_> = raw::array(graph.projection(), "tagPaths")?
        .iter()
        .filter(|row| row.get("source") == Some(&reference))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "one actual tag interpretation for the selected source occurrence"
    );
    let row = *matching.first().ok_or("actual selected original tag")?;
    let path = row.get("path").ok_or("independently sourced tag path")?;
    if let Some(expected) = expected {
        assert_eq!(
            row.get("interpretation"),
            Some(&json!("path")),
            "the genuine selected TagTree path"
        );
        assert_eq!(
            raw::present(path)?,
            &json!(expected),
            "the complete original selected path"
        );
    } else {
        assert!(
            matches!(
                row.get("interpretation").and_then(Value::as_str),
                Some("null" | "inapplicable")
            ),
            "actual null or nil remains an explicit lookup decision"
        );
        assert_eq!(
            path.get("state"),
            Some(&json!("absent")),
            "no invented path for nil/null"
        );
        assert_eq!(
            path.get("value"),
            Some(&Value::Null),
            "the absent projection has no path payload"
        );
    }
    Ok(())
}
