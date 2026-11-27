// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{IniEncoding, InputScope, Language, LocalizationEntry, LocalizationSet};
use serde_json::{Value, json};

use super::TestResult;
use super::origins::{TextOrigins, encryption_marker, rows};
use crate::catalog_v3_consumer::{present, source_value};

pub(super) fn verify(
    catalog: &Value,
    locales: &LocalizationSet,
    evidence: &TextOrigins<'_>,
    directory_entries: usize,
) -> TestResult<()> {
    if catalog.pointer("/texts/languagePolicy")
        != Some(&json!({"selection":"availableSupported", "fallback":"none"}))
    {
        return Err("the catalog must declare its supported-language and fallback policy".into());
    }
    let sources = catalog
        .pointer("/texts/languageSources")
        .and_then(Value::as_object)
        .ok_or("catalog-wide language source map")?;
    if sources.len() != Language::ALL.len() {
        return Err("the source map must classify every supported language".into());
    }
    for language in Language::ALL {
        let cell = sources
            .get(language.code())
            .ok_or("supported language source cell")?;
        locales
            .entry(language)
            .map_or_else(
                || verify_absence(cell, evidence, directory_entries),
                |entry| verify_loaded(cell, entry, evidence),
            )
            .map_err(|error| format!("{language}: {error}"))?;
    }
    Ok(())
}

fn verify_loaded(
    cell: &Value,
    entry: &LocalizationEntry,
    evidence: &TextOrigins<'_>,
) -> TestResult<()> {
    let observed = entry.origin().ok_or("the original locale Entry binding")?;
    let marker = encryption_marker(observed.encryption());
    verify_marker(cell, marker)?;
    let source = present(cell)?;
    let union = rows(cell, "/sourceUsesRefs")?;
    let text = entry.text();
    let fields = [
        ("path", json!(entry.path())),
        ("encoding", json!(encoding_name(text.encoding())?)),
        ("lossyDecoding", json!(text.is_lossy())),
        ("skippedMalformedLines", json!(skipped_lines(text.text()))),
    ];
    if union.len() != fields.len() {
        return Err("the language cell must cover all four original observations".into());
    }
    for (field, expected) in fields {
        let carrier = source.get(field).ok_or("sourced locale field")?;
        if present(carrier)? != &expected {
            return Err(format!("{field} differs from the loaded original Entry").into());
        }
        let [reference] = rows(carrier, "/sourceUsesRefs")? else {
            return Err("each language field must cite one direct observation".into());
        };
        if !union.contains(reference) {
            return Err("the outer language source omits a field observation".into());
        }
        let used = evidence.use_site(reference)?;
        let (scope, field_marker, role, location) = if field == "path" {
            (
                InputScope::ArchiveDirectoryMetadata,
                Some(false),
                "payload",
                json!({"type":"metadata", "field":field}),
            )
        } else {
            (
                InputScope::Payload,
                marker,
                "conversion",
                json!({"type":"subresource", "namespace":"globalIni", "indices":[], "field":field}),
            )
        };
        verify_marker(carrier, field_marker)?;
        evidence.physical_source(used, observed, scope)?;
        if used.get("role").and_then(Value::as_str) != Some(role)
            || used.get("location") != Some(&location)
        {
            return Err(format!("{field} cites an unrelated original operation").into());
        }
    }
    Ok(())
}

fn verify_absence(
    cell: &Value,
    evidence: &TextOrigins<'_>,
    directory_entries: usize,
) -> TestResult<()> {
    if source_value(cell)?.is_some() || cell.get("state").and_then(Value::as_str) != Some("absent")
    {
        return Err("a language missing from the Archive must have an absent source cell".into());
    }
    verify_marker(cell, Some(false))?;
    let [reference] = rows(cell, "/sourceUsesRefs")? else {
        return Err("one original root directory survey is required".into());
    };
    let used = evidence.use_site(reference)?;
    let source = evidence.source(used)?;
    let end = directory_entries.to_string();
    let location = json!({
        "type": "archiveDirectoryPaths",
        "startEntryIndex": "0",
        "endEntryIndex": end,
    });
    if used.get("scope") != Some(&json!(InputScope::ArchiveDirectoryMetadata))
        || used.get("role").and_then(Value::as_str) != Some("selector")
        || used.get("location") != Some(&location)
        || source.get("kind").and_then(Value::as_str) != Some("archiveDirectory")
        || source.get("admittedEntryCount").and_then(Value::as_str) != Some(end.as_str())
    {
        return Err("the language's absence lacks the full original directory survey".into());
    }
    Ok(())
}

fn verify_marker(cell: &Value, marker: Option<bool>) -> TestResult<()> {
    if cell.get("containsEncryptedSource") != Some(&json!(marker))
        || cell.get("sourceSetComplete").and_then(Value::as_bool) != Some(true)
    {
        return Err("the language's exact encryption and source-proof state must survive".into());
    }
    Ok(())
}

fn encoding_name(value: IniEncoding) -> TestResult<&'static str> {
    Ok(match value {
        IniEncoding::Utf8 => "utf8",
        IniEncoding::Utf8Bom => "utf8Bom",
        IniEncoding::Utf16Le => "utf16Le",
        IniEncoding::Utf16Be => "utf16Be",
        _ => return Err("no wire encoding for the selected original decoder".into()),
    })
}

fn skipped_lines(text: &str) -> usize {
    text.strip_prefix('\u{feff}')
        .unwrap_or(text)
        .lines()
        .filter(|line| {
            !line.trim().is_empty()
                && !line.split_once('=').is_some_and(|(key, _)| {
                    let head = key.split_once(',').map_or(key, |(head, _)| head);
                    !head.trim().is_empty()
                })
        })
        .count()
}
