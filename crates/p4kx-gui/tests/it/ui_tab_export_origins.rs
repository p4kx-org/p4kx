// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use p4kx_gui::UiExportSummary;
use serde_json::Value;

use super::TestResult;

pub(super) fn assert_receipt(
    output: &Path,
    summary: &UiExportSummary,
) -> Result<Vec<Value>, Box<dyn Error>> {
    let name = output.file_name().ok_or("output basename")?;
    let mut companion = name.to_os_string();
    companion.push(".p4kx-origin.jsonl");
    assert_eq!(
        fs::canonicalize(summary.origin_receipt())?,
        fs::canonicalize(output.with_file_name(companion))?,
        "the summary exposes the committed sibling"
    );
    let receipt = fs::read_to_string(summary.origin_receipt())?;
    let rows: Vec<Value> = receipt
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let header = rows.first().ok_or("companion header")?;
    let mut digest = OriginDigestWriter::new(io::sink());
    digest.write_all(&fs::read(output)?)?;
    let expected = serde_json::to_value(digest.finish()?)?;
    assert_eq!(
        header
            .pointer("/binding/path/value")
            .and_then(Value::as_str),
        name.to_str(),
        "relative output name"
    );
    for field in ["sha256", "byteLength"] {
        assert_eq!(
            header.get("binding").and_then(|binding| binding.get(field)),
            expected.get(field),
            "committed output {field}"
        );
    }
    assert_eq!(
        rows.last()
            .and_then(|row| row.get("type"))
            .and_then(Value::as_str),
        Some("end"),
        "complete companion"
    );
    assert_summary(summary, &rows)?;
    Ok(rows)
}

fn assert_summary(summary: &UiExportSummary, rows: &[Value]) -> TestResult {
    let origins = summary.origins();
    let artifact = rows
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .ok_or("encoded artifact field")?;
    let serialized = serde_json::to_value(&origins.artifact)?;
    for field in [
        "containsEncryptedSource",
        "sourceUsesRefs",
        "sourceSetComplete",
    ] {
        assert_eq!(
            artifact.get(field),
            serialized.get(field),
            "artifact {field}"
        );
    }
    assert!(
        !origins.inventory.uses().is_empty(),
        "the physical uses are retained"
    );
    assert_eq!(
        origins.file.contains_encrypted_source(),
        Some(false),
        "caller path"
    );
    assert!(
        origins.file.source_uses_refs().is_empty(),
        "caller path has no game input"
    );
    for dimension in [&origins.width, &origins.height] {
        assert_eq!(
            dimension.contains_encrypted_source(),
            Some(false),
            "clear stage"
        );
        assert!(
            !dimension.source_uses_refs().is_empty(),
            "authored stage dimension"
        );
    }
    assert_eq!(
        origins.left_out.contains_encrypted_source(),
        None,
        "unbound omission count"
    );
    Ok(())
}

pub(super) fn assert_screen_sources(rows: &[Value]) -> TestResult {
    let artifact = rows
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .ok_or("encoded artifact field")?;
    let references = artifact
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("artifact use references")?;
    for (namespace, path) in [
        ("dataForgeBytes", "Data/Game2.dcb"),
        ("gfxTag", "Data/UI/fonts/Shared/fonts_en.gfx"),
        ("globalIniBytes", "Data/Localization/english/global.ini"),
    ] {
        let source_use = rows
            .iter()
            .filter_map(|row| row.get("sourceUse"))
            .find(|usage| {
                usage.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                    && usage.get("id").is_some_and(|id| references.contains(id))
            })
            .ok_or_else(|| format!("artifact use {namespace}"))?;
        let physical = rows
            .iter()
            .filter_map(|row| row.get("source"))
            .find(|source| source.get("id") == source_use.get("source"))
            .and_then(|source| source.pointer("/path/value"))
            .and_then(Value::as_str)
            .ok_or("observed Archive Entry path")?;
        assert_eq!(
            physical.replace('\\', "/"),
            path,
            "actual source of {namespace}"
        );
    }
    Ok(())
}
