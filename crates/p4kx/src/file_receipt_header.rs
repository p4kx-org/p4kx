// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use p4kx_extraction::{OriginArtifactName, SOURCE_ORIGIN_FORMAT, SOURCE_ORIGIN_VERSION};
use serde_json::{Value, json};

use super::lock::{invalid, regular_file};

#[derive(Clone, Copy)]
pub(super) enum ReceiptOwner<'name> {
    File(&'name OriginArtifactName),
    Stdout,
}

pub(super) fn check_existing(path: &Path, owner: ReceiptOwner<'_>) -> io::Result<bool> {
    if regular_file(path)?.is_none() {
        return Ok(false);
    }
    let mut binding = match owner {
        ReceiptOwner::File(path) => json!({"type": "file", "path": path}),
        ReceiptOwner::Stdout => json!({"type": "stdout"}),
    };
    let shape = binding
        .as_object_mut()
        .ok_or_else(|| invalid("invalid origin binding"))?;
    shape.insert("byteLength".to_owned(), Value::String(u64::MAX.to_string()));
    shape.insert("sha256".to_owned(), Value::String("f".repeat(64)));
    let expected = json!({
        "type": "header",
        "format": SOURCE_ORIGIN_FORMAT,
        "formatVersion": SOURCE_ORIGIN_VERSION,
        "binding": binding,
    });
    let bound = serde_json::to_vec(&expected)?.len() as u64 + 1;
    let mut reader = BufReader::new(File::open(path)?).take(bound);
    let mut line = Vec::new();
    reader.read_until(b'\n', &mut line)?;
    let Some((b'\n', json)) = line.split_last() else {
        return Err(invalid("the existing companion has no valid origin header"));
    };
    let mut actual: Value = serde_json::from_slice(json)
        .map_err(|_| invalid("the existing companion has no valid origin header"))?;
    if !normalize_measurement(&mut actual) || actual != expected {
        return Err(invalid(
            "the existing companion belongs to another artifact or format",
        ));
    }
    Ok(true)
}

fn normalize_measurement(header: &mut Value) -> bool {
    let Some(binding) = header.get_mut("binding").and_then(Value::as_object_mut) else {
        return false;
    };
    let Some(length) = binding.get_mut("byteLength") else {
        return false;
    };
    let Some(bytes) = length.as_str() else {
        return false;
    };
    let Ok(count) = bytes.parse::<u64>() else {
        return false;
    };
    if bytes != count.to_string() {
        return false;
    }
    *length = Value::String(u64::MAX.to_string());
    let Some(hash) = binding.get_mut("sha256") else {
        return false;
    };
    let Some(value) = hash.as_str() else {
        return false;
    };
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return false;
    }
    *hash = Value::String("f".repeat(64));
    true
}
