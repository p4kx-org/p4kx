// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::{BASE, BASE_PATH, DCB, LOCAL, LOCAL_PATH, ROOT, original_bytes};
use crate::audio_origin_assertions as proof;
use crate::support::TestResult;

pub(super) fn check(page: &Value, name: &str, database: &[u8]) -> TestResult {
    match name {
        "left/wing" => check_route(page, BASE_PATH, BASE, "left/wing", &[], database, 2),
        "right" => check_route(page, LOCAL_PATH, LOCAL, "right", &[], database, 1),
        "mount" => check_route(page, LOCAL_PATH, LOCAL, "right", &["mount"], database, 2),
        "capsule" => check_route(
            page,
            LOCAL_PATH,
            LOCAL,
            "right",
            &["mount", "capsule"],
            database,
            3,
        ),
        _ => Err("unexpected port in the loadout route fixture".into()),
    }
}

fn check_route(
    page: &Value,
    entry: &str,
    xml: &str,
    parent: &str,
    record_names: &[&str],
    database: &[u8],
    depth: usize,
) -> TestResult {
    assert_eq!(
        page.pointer("/ports/0/depth"),
        Some(&json!(depth)),
        "existing depth"
    );
    let path = require_paired_origins(page)?;
    proof::assert_marker(page, &path, entry == BASE_PATH || !record_names.is_empty())?;
    let mut found = Vec::new();
    for site in proof::uses(page, &path)? {
        let field = site.pointer("/location/namespace").and_then(Value::as_str);
        match field {
            Some("textXmlAttribute") => {
                assert_eq!(proof::source_path(page, site)?, entry, "selected XML entry");
                assert_eq!(
                    original_bytes(xml.as_bytes(), site)?,
                    format!("portName='{parent}'").as_bytes(),
                    "exact original XML name attribute"
                );
                assert_eq!(
                    site.get("role"),
                    Some(&json!("conversion")),
                    "name derived from this authored value"
                );
                found.push(parent.as_bytes());
            }
            Some("dataforgeDecodedBytes") => {
                assert_eq!(
                    proof::source_path(page, site)?,
                    DCB,
                    "selected DataForge entry"
                );
                assert_eq!(
                    site.get("role"),
                    Some(&json!("conversion")),
                    "name derived from this authored value"
                );
                found.push(original_bytes(database, site)?);
            }
            Some("dataforgeValue") => assert_eq!(
                site.get("role"),
                Some(&json!("selector")),
                "DataForge name field selector"
            ),
            _ => return Err("unexpected route contributor".into()),
        }
    }
    let mut expected: Vec<_> = std::iter::once(parent)
        .chain(record_names.iter().copied())
        .map(str::as_bytes)
        .collect();
    found.sort_unstable();
    expected.sort_unstable();
    assert_eq!(
        found, expected,
        "exact selected names across the original route"
    );
    Ok(())
}

fn require_paired_origins(page: &Value) -> TestResult<String> {
    let path = format!("{ROOT}/ports/0/path");
    let slash_count = format!("{ROOT}/ports/0/depth");
    let origins = page.get("fieldOrigins").ok_or("route origins")?;
    assert_eq!(
        origins.get(&path),
        origins.get(&slash_count),
        "both outputs use the same names"
    );
    Ok(path)
}
