// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Checks extraction payloads and their portable source-origin companions.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use serde_json::{Value, json};

use crate::common::{TestResult, files_below};

/// Checks the exact set of payload files and their byte-bound sidecars.
pub(crate) fn assert_receipted_outputs(dir: &Path, receipted: &[&str]) -> TestResult {
    assert_outputs(dir, receipted, &[])
}

/// Checks the exact set of files including payloads that have no standalone sidecar.
pub(crate) fn assert_outputs(dir: &Path, receipted: &[&str], additional: &[&str]) -> TestResult {
    let mut expected: Vec<String> = additional.iter().map(ToString::to_string).collect();
    for &relative in receipted {
        expected.push(relative.to_owned());
        expected.push(receipt_name(relative));
    }
    expected.sort_unstable();
    assert_eq!(
        files_below(dir)?,
        expected,
        "the exact extraction outputs below {}",
        dir.display()
    );

    for &relative in receipted {
        assert_receipt(dir, relative)?;
    }
    Ok(())
}

fn receipt_name(relative: &str) -> String {
    format!("{relative}.p4kx-origin.jsonl")
}

fn assert_receipt(dir: &Path, relative: &str) -> TestResult {
    let receipt = fs::read_to_string(dir.join(receipt_name(relative).replace('\\', "/")))?;
    let rows = receipt
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let [header, body @ .., end] = rows.as_slice() else {
        return Err(format!("incomplete origin receipt for {relative}").into());
    };

    let path = dir.join(relative.replace('\\', "/"));
    let basename = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or("expected a portable test output basename")?;
    let mut digest = OriginDigestWriter::new(io::sink());
    io::copy(&mut File::open(&path)?, &mut digest)?;
    let digest = digest.finish()?;
    assert_eq!(
        header,
        &json!({
            "type": "header",
            "format": "p4kx-source-origin",
            "formatVersion": "1.0",
            "binding": {
                "type": "file",
                "path": { "encoding": "utf8", "value": basename },
                "byteLength": digest.byte_length().to_string(),
                "sha256": digest.sha256(),
            }
        }),
        "the origin receipt binds the exact bytes at {relative}"
    );

    assert_complete(body, end, relative)
}

fn assert_complete(body: &[Value], end: &Value, relative: &str) -> TestResult {
    let (mut sources, mut source_uses, mut fields) = (0, 0, 0);
    for row in body {
        match row.get("type").and_then(Value::as_str) {
            Some("source") => sources += 1,
            Some("sourceUse") => source_uses += 1,
            Some("field") => fields += 1,
            _ => return Err(format!("unexpected receipt row for {relative}: {row:?}").into()),
        }
    }
    assert!(
        sources > 0 && source_uses > 0 && fields > 0,
        "the receipt carries original evidence for {relative}"
    );
    assert_eq!(
        end,
        &json!({
            "type": "end",
            "sources": sources.to_string(),
            "sourceUses": source_uses.to_string(),
            "fields": fields.to_string(),
        }),
        "the receipt has a matching completion record for {relative}"
    );
    Ok(())
}
