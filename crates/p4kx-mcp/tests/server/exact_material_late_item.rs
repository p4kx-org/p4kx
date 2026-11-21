// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::exact_material_assertions as check;
use crate::exact_material_fixture::{self as fixture, Encoding};
use crate::support::TestResult;

#[test]
fn exact_material_late_oversize_keeps_the_prefix_and_retries_the_exact_item() -> TestResult {
    let huge = "z".repeat(300 * 1024);
    let library = fixture::parameter_library(&[("first", "kept"), ("second", &huge)]);
    let install = fixture::install(&library, Encoding::Text, Vec::new())?;
    let original = fixture::original(&install)?;
    assert_eq!(
        original.materials.len(),
        2,
        "both actual original source items exist"
    );
    let later = original
        .materials
        .get(1)
        .ok_or("original second material")?;
    assert_eq!(later.name, "second", "actual original order");
    assert_eq!(
        later.public_params.first().map(|(_, value)| value),
        Some(&huge),
        "the actual second value is fully present"
    );
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.ok("mtl_summary", &json!({"path":fixture::MATERIAL}))?;
    assert_eq!(
        first
            .get("materials")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "a later oversized item cannot erase the complete first item or be shortened into the page"
    );
    assert_eq!(
        check::material(&first)?.get("id"),
        Some(&json!(0)),
        "the complete original prefix retains its identity"
    );
    let cursor = first
        .get("next_cursor")
        .and_then(Value::as_str)
        .ok_or("cursor on exactly the oversized second item")?;
    check::too_large(
        &mut server,
        &json!({"path":fixture::MATERIAL,"cursor":cursor}),
    )
}
