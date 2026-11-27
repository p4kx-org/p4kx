// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::commodity_v3_fixture;
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::spawns_fixture;
use crate::template_canonical_graph::{Template, condition};

pub(crate) fn check(catalog: &Value) -> TestResult {
    let identity = commodity_v3_fixture::guid(spawns_fixture::DEEP_FLOW);
    let graph = Template::new(catalog, &identity)?;
    let mut node = graph.condition()?;
    let mut nots = 0;
    while condition(node)?.get("type") == Some(&json!("not")) {
        nots += 1;
        node = graph.owner.follow(
            condition(node)?
                .get("child")
                .ok_or("actual original NOT child")?,
        )?;
    }
    assert_eq!(
        nots, 18,
        "the entire existing original 18-NOT source chain survives"
    );
    assert_eq!(
        condition(node)?.get("type"),
        Some(&json!("tokenState")),
        "the actual leaf condition"
    );
    assert_eq!(
        condition(node)?.get("state"),
        Some(&json!({"type":"known","state":"active"})),
        "the original leaf state interpretation"
    );
    let roundtrip: Value = serde_json::from_slice(&serde_json::to_vec(catalog)?)?;
    assert_eq!(
        &roundtrip, catalog,
        "the whole canonical envelope survives default JSON parsing"
    );
    assert!(
        !raw::array(graph.owner.source, "values")?.is_empty(),
        "real source occurrences are retained"
    );
    Ok(())
}
