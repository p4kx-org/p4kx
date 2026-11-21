// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_media::MediaRecord;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{TestResult, attachment, frame_origin, json_rows, placement};

pub(super) fn check(rows: &[MediaRecord], base: &Path) -> TestResult {
    let (model_ordinal, model) = attachment(rows, 0, 6)?;
    assert_eq!(
        model.data.pointer("/payload/model_position"),
        Some(&json!({"authored":"100,200,300","value":[100.0,200.0,300.0]})),
        "preserve the authored model translation and decode it in metres"
    );
    check_matrix(
        model,
        [
            [1.0, 0.0, 0.0, 100.0],
            [0.0, 1.0, 0.0, 200.0],
            [0.0, 0.0, 1.0, 300.0],
        ],
    )?;

    let (mixed_ordinal, mixed) = attachment(rows, 0, 7)?;
    assert_eq!(
        mixed.data.pointer("/payload/model_rotation_wxyz"),
        Some(&json!({"authored":"invalid shadowed","value":null})),
        "the shadowed malformed value remains addressable"
    );
    check_matrix(
        mixed,
        [
            [0.0, -1.0, 0.0, 2.0],
            [1.0, 0.0, 0.0, 3.0],
            [0.0, 0.0, 1.0, 4.0],
        ],
    )?;

    let uses = json_rows(base, "origin-uses.jsonl")?;
    check_inputs(base, model_ordinal, &uses, "Rotation", false)?;
    check_inputs(base, mixed_ordinal, &uses, "RelRotation", true)?;
    check_default_selector(rows, base, &uses)
}

fn check_default_selector(rows: &[MediaRecord], base: &Path, uses: &[Value]) -> TestResult {
    let (ordinal, row) = attachment(rows, 0, 8)?;
    check_matrix(
        row,
        [
            [0.0, -1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
    )?;
    let selected = frame_uses(base, ordinal, uses)?;
    let fields: Vec<_> = selected
        .into_iter()
        .filter(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some("cdf-xml-attribute")
        })
        .filter_map(|used| {
            let field = used.pointer("/location/field").and_then(Value::as_str)?;
            let role = used.get("role").and_then(Value::as_str)?;
            matches!(
                field,
                "Position" | "Rotation" | "RelPosition" | "RelRotation"
            )
            .then_some((field, role))
        })
        .collect();
    assert_eq!(
        fields.len(),
        2,
        "one operative rotation and one default frame selector"
    );
    assert!(
        fields.contains(&("Rotation", "selector")) && fields.contains(&("RelRotation", "payload")),
        "shadowed attribute provenance is structural: {fields:?}"
    );
    Ok(())
}

fn check_matrix(row: &MediaRecord, expected: [[f64; 4]; 3]) -> TestResult {
    let point = placement(row)?;
    assert_eq!(
        point.get("disposition"),
        Some(&json!("bound")),
        "the fixture verifies the same unique declared rig"
    );
    let frame = point.get("rig_from_attachment").ok_or("rig-space frame")?;
    let matrix = <[[f64; 4]; 3]>::deserialize(frame)?;
    for (actual, expected) in matrix
        .into_iter()
        .flatten()
        .zip(expected.into_iter().flatten())
    {
        assert!(
            (actual - expected).abs() < 1e-5,
            "a model-space component is placed once: {actual} vs {expected}"
        );
    }
    Ok(())
}

fn frame_uses<'a>(
    base: &Path,
    ordinal: usize,
    uses: &'a [Value],
) -> Result<Vec<&'a Value>, Box<dyn std::error::Error>> {
    let frame = frame_origin(base, ordinal)?;
    let refs = frame
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("selected pose sources")?;
    Ok(uses
        .iter()
        .filter(|used| used.get("id").is_some_and(|id| refs.contains(id)))
        .collect())
}

fn check_inputs(
    base: &Path,
    ordinal: usize,
    uses: &[Value],
    expected_rotation: &str,
    expect_bind_pose: bool,
) -> TestResult {
    let locations: Vec<_> = frame_uses(base, ordinal, uses)?
        .into_iter()
        .filter_map(|used| used.get("location"))
        .collect();
    let authored: Vec<_> = locations
        .iter()
        .filter(|site| site.get("namespace").and_then(Value::as_str) == Some("cdf-xml-attribute"))
        .filter_map(|site| site.get("field").and_then(Value::as_str))
        .filter(|field| {
            matches!(
                *field,
                "Position" | "Rotation" | "RelPosition" | "RelRotation"
            )
        })
        .collect();
    assert_eq!(authored.len(), 2, "only the two winning source fields");
    assert!(
        authored.contains(&"Position") && authored.contains(&expected_rotation),
        "exact contributing CDF positions: {authored:?}"
    );
    let binds = locations.iter().any(|site| {
        site.get("namespace").and_then(Value::as_str) == Some("model-skeleton-bone")
            && site.get("field").and_then(Value::as_str) == Some("relativePose")
    });
    assert_eq!(
        binds, expect_bind_pose,
        "only the mixed pose consumes the physical bind coefficients"
    );
    Ok(())
}
