// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_media::{Identity, Index, MediaRecord};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::support::TestResult;

pub(super) fn field<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    value
        .pointer(path)
        .ok_or_else(|| format!("required field {path}").into())
}

pub(super) fn number(value: &Value, path: &str) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(field(value, path)?
        .as_str()
        .ok_or("canonical decimal text")?
        .parse()?)
}

pub(super) fn row<'a>(
    rows: &'a [MediaRecord],
    namespace: &str,
) -> Result<&'a MediaRecord, Box<dyn std::error::Error>> {
    rows.iter()
        .find(|row| row.subresource.namespace == namespace)
        .ok_or_else(|| format!("missing {namespace} row").into())
}

pub(super) fn raw(
    output: &Path,
    index: &Index,
    rows: &[MediaRecord],
    source: &Identity,
    reference: &Value,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let series = field(reference, "/series")?;
    let start = number(reference, "/range/offset")?;
    let length = number(reference, "/range/length")?;
    let end = start.checked_add(length).ok_or("raw reference overflow")?;
    let mut result = Vec::new();
    let mut position = start;
    for row in rows.iter().filter(|row| {
        row.subresource.namespace == "archive-raw-chunk" && &row.source.identity == source
    }) {
        if field(&row.data, "/series")? != series {
            continue;
        }
        let part = number(&row.data, "/range/offset")?;
        let size = number(&row.data, "/range/length")?;
        let bytes = artifact(output, index, &row.data)?;
        assert_eq!(
            u64::try_from(bytes.len())?,
            size,
            "exact sidecar range length"
        );
        let left = part.max(start);
        let right = part
            .checked_add(size)
            .ok_or("chunk extent overflow")?
            .min(end);
        if left >= right {
            continue;
        }
        assert_eq!(
            left, position,
            "raw reference is contiguous without missing or repeated bytes"
        );
        result.extend_from_slice(
            bytes
                .get(usize::try_from(left - part)?..usize::try_from(right - part)?)
                .ok_or("chunk subrange")?,
        );
        position = right;
    }
    assert_eq!(
        position, end,
        "every nonempty byte of the reference is retained"
    );
    Ok(result)
}

fn artifact(
    output: &Path,
    index: &Index,
    data: &Value,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let path = field(data, "/chunk/artifact/path")?
        .as_str()
        .ok_or("artifact path")?;
    let bytes = fs::read(output.join(&index.artifact_base).join(path))?;
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let actual = p4kx_extraction::Checksum::Sha256(digest).hex();
    assert_eq!(
        field(data, "/chunk/artifact/digest/hex")?.as_str(),
        Some(actual.as_str()),
        "hash of actual sidecar bytes"
    );
    assert_eq!(
        field(data, "/chunk/artifact/size")?.as_u64(),
        Some(u64::try_from(bytes.len())?),
        "actual sidecar length"
    );
    Ok(bytes)
}

pub(super) fn validate(rows: &[MediaRecord]) -> TestResult {
    let schema = p4kx_media::builtin_record_schema()?;
    let validator = jsonschema::validator_for(&schema)?;
    for row in rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
