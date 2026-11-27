// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_media::{Identity, MediaRecord};
use p4kx_testkit::TestSkeleton;
use serde_json::{Value, json};

use super::model_glb_fixture::HELPER;
use super::model_source_provider::field_uses;
use super::support::TestResult;

pub(super) fn check_metadata(rows: &[MediaRecord]) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    let (_, helper) = physical_row(rows, 0, "model-node", &[2, 0])?;
    for (field, value) in [
        ("name/display_name_utf8", json!(HELPER)),
        ("helper", json!(true)),
        ("local_matrix_bits/0/1", json!((-1.0_f32).to_bits())),
        ("local_matrix_bits/1/3", json!(2.0_f32.to_bits())),
    ] {
        check_field(helper, field, &value);
    }
    let (_, property) = physical_row(rows, 0, "model-node-property", &[2, 0, 0])?;
    check_field(
        property,
        "assignment",
        &json!({"key": "component", "value": "inspection"}),
    );
    for (index, disposition) in [(5, "known-opaque"), (6, "unknown-type")] {
        let (_, chunk) = physical_row(rows, 0, "model-chunk", &[index])?;
        check_field(chunk, "projection", &json!(disposition));
    }
    for (owner, chunk, skeleton) in [
        (0, 4, TestSkeleton::two_bones()),
        (2, 1, TestSkeleton::chain(6)),
    ] {
        let (_, table) = physical_row(rows, owner, "model-skeleton-chunk", &[chunk])?;
        check_field(table, "bone_count", &json!(skeleton.bones().len()));
        for (ordinal, bone) in skeleton.bones().iter().enumerate() {
            let (_, row) = physical_row(
                rows,
                owner,
                "model-skeleton-bone",
                &[chunk, u64::try_from(ordinal)?],
            )?;
            check_field(row, "name", &json!(bone.name));
            check_field(row, "parent", &json!(bone.parent));
            check_field(
                row,
                "relative/translation_z_up_metres",
                &json!(bone.translation),
            );
        }
    }
    Ok(())
}

pub(super) fn original_vertex_order(
    generation: &Path,
    rows: &[MediaRecord],
    owner: usize,
    width: usize,
) -> Result<Vec<u32>, Box<dyn Error>> {
    let mut order = Vec::new();
    for ordinal in 0..2 {
        let (row_ordinal, row) = physical_row(rows, owner, "model-mesh-submesh", &[0, ordinal])?;
        let bounds = original_index_bounds(generation, row_ordinal, row, owner, width)?;
        let raw = row.source.raw.as_ref().ok_or("retained geometry")?;
        let bytes = fs::read(generation.join(&raw.path))?;
        let page = usize::try_from(number(row, "vertex_page_base")?)?;
        for bytes in bytes
            .get(bounds)
            .ok_or("physical source indices")?
            .chunks_exact(width)
        {
            let vertex = match *bytes {
                [a, b] => u32::from(u16::from_le_bytes([a, b])),
                [a, b, c, d] => u32::from_le_bytes([a, b, c, d]),
                _ => return Err("source index width".into()),
            };
            let absolute = vertex + u32::try_from(page)?;
            if !order.contains(&absolute) {
                order.push(absolute);
            }
        }
    }
    Ok(order)
}

fn original_index_bounds(
    generation: &Path,
    ordinal: usize,
    row: &MediaRecord,
    owner: usize,
    width: usize,
) -> Result<std::ops::Range<usize>, Box<dyn Error>> {
    check_field(row, "source_vertices/stride_bytes", &json!(width));
    let start = number(row, "source_vertices/indices/start")?;
    let end = number(row, "source_vertices/indices/end_exclusive")?;
    assert_eq!(
        end - start,
        number(row, "index_count")? * u64::try_from(width)?,
        "complete authored index interval"
    );
    let uses = field_uses(
        generation,
        ordinal,
        "/data/payload/source_vertices/indices/start",
        owner,
        width == 4,
    )?;
    let span = json!([start.to_string(), end.to_string()]);
    assert!(
        uses.iter()
            .any(|usage| usage.pointer("/location/indices") == Some(&span)),
        "stored indices field refers to the original bytes"
    );
    Ok(usize::try_from(start)?..usize::try_from(end)?)
}

fn number(row: &MediaRecord, field: &str) -> Result<u64, Box<dyn Error>> {
    row.data
        .pointer(&format!("/payload/{field}"))
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing unsigned Media field {field}").into())
}

pub(super) fn physical_row<'a>(
    rows: &'a [MediaRecord],
    owner: usize,
    namespace: &str,
    indices: &[u64],
) -> Result<(usize, &'a MediaRecord), Box<dyn Error>> {
    rows.iter().enumerate().find(|(_, row)| {
        matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.entry_index == owner)
            && row.subresource.namespace == namespace
            && row.subresource.indices == indices
    }).ok_or_else(|| format!("source Entry {owner} namespace {namespace} indices {indices:?}").into())
}

pub(super) fn check_field(row: &MediaRecord, field: &str, expected: &Value) {
    assert_eq!(
        row.data.pointer(&format!("/payload/{field}")),
        Some(expected),
        "original media field {field}"
    );
}
