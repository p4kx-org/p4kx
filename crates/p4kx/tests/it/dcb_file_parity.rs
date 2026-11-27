// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::{self, Write};
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use serde_json::Value;

use super::{TestResult, files};

pub(super) fn assert_record_export_parity(
    from_file: &Path,
    from_archive: &Path,
    language: Option<&str>,
) -> TestResult {
    let file_outputs = files(from_file)?;
    let archive_outputs = files(from_archive)?;
    for outputs in [&file_outputs, &archive_outputs] {
        assert_eq!(outputs.len(), 4, "two Record XML files and two companions");
    }
    for title in ["Cart", "Truck"] {
        let path = format!(
            "libs/foundry/records/vehicles/{}.xml",
            title.to_ascii_lowercase()
        );
        let payload_path = Path::new(&path);
        let file = file_outputs.get(payload_path).ok_or("loose-input XML")?;
        let archive = archive_outputs
            .get(payload_path)
            .ok_or("Archive-input XML")?;
        assert_eq!(file, archive, "{path} has identical emitted bytes");

        let companion = format!("{path}.p4kx-origin.jsonl");
        let companion_path = Path::new(&companion);
        let loose = file_outputs
            .get(companion_path)
            .ok_or("loose-input receipt")?;
        let entry = archive_outputs
            .get(companion_path)
            .ok_or("Archive-input receipt")?;
        let key = format!("@vehicle_{title}");
        let translation = language.map(|language| (language, title));
        assert_receipt(file, loose, Input::LooseFile, &key, translation)?;
        assert_receipt(archive, entry, Input::ArchiveEntry, &key, translation)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Input {
    LooseFile,
    ArchiveEntry,
}

fn assert_receipt(
    payload: &[u8],
    receipt: &[u8],
    input: Input,
    key: &str,
    translation: Option<(&str, &str)>,
) -> TestResult {
    let rows = str::from_utf8(receipt)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    assert_binding(&rows, payload)?;
    let (kind, address, name, marker) = match input {
        Input::LooseFile => ("file", "/name/value", "Game2.dcb", Value::Null),
        Input::ArchiveEntry => (
            "archiveEntry",
            "/path/value",
            r"Data\Game2.dcb",
            Value::Bool(false),
        ),
    };
    let definition = source_id(&rows, kind, address, name)?;
    let key_field = attribute(&rows, payload, key)?;
    assert_eq!(
        key_field.get("containsEncryptedSource"),
        Some(&marker),
        "{name} supplies the key's ancestry"
    );
    assert!(
        field_uses(&rows, key_field)?.iter().any(|use_row| {
            use_row.get("source") == Some(definition)
                && use_row
                    .pointer("/location/namespace")
                    .and_then(Value::as_str)
                    == Some("dataforgeDecodedBytes")
        }),
        "{name} supplies the original key bytes"
    );
    if let Some((language, title)) = translation {
        let field = attribute(&rows, payload, title)?;
        assert_translation(&rows, field, definition, language)?;
    }
    Ok(())
}

fn assert_binding(rows: &[Value], payload: &[u8]) -> TestResult {
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(payload)?;
    let digest = serde_json::to_value(measured.finish()?)?;
    let header = rows.first().ok_or("the receipt header")?;
    assert_eq!(
        header.pointer("/binding/byteLength"),
        digest.get("byteLength"),
        "the receipt binds the emitted XML length"
    );
    assert_eq!(
        header.pointer("/binding/sha256"),
        digest.get("sha256"),
        "the receipt binds the emitted XML digest"
    );
    assert_eq!(
        rows.last()
            .and_then(|row| row.get("type"))
            .and_then(Value::as_str),
        Some("end"),
        "the receipt is complete"
    );
    Ok(())
}

fn source_id<'rows>(
    rows: &'rows [Value],
    kind: &str,
    address: &str,
    name: &str,
) -> Result<&'rows Value, Box<dyn Error>> {
    rows.iter()
        .filter_map(|row| row.get("source"))
        .find(|source| {
            source.get("kind").and_then(Value::as_str) == Some(kind)
                && source.pointer(address).and_then(Value::as_str) == Some(name)
        })
        .and_then(|source| source.get("id"))
        .ok_or_else(|| format!("missing physical source {kind} {name}").into())
}

fn attribute<'rows>(
    rows: &'rows [Value],
    payload: &[u8],
    value: &str,
) -> Result<&'rows Value, Box<dyn Error>> {
    rows.iter()
        .find(|row| {
            row.pointer("/address/type").and_then(Value::as_str) == Some("xmlBytes")
                && row.pointer("/address/field").and_then(Value::as_str) == Some("attributeValue")
                && addressed_bytes(row, payload) == Some(value.as_bytes())
        })
        .ok_or_else(|| format!("missing XML attribute value {value}").into())
}

fn addressed_bytes<'payload>(row: &Value, payload: &'payload [u8]) -> Option<&'payload [u8]> {
    let start = row
        .pointer("/address/start")?
        .as_str()?
        .parse::<usize>()
        .ok()?;
    let end = row
        .pointer("/address/endExclusive")?
        .as_str()?
        .parse::<usize>()
        .ok()?;
    payload.get(start..end)
}

fn field_uses<'rows>(
    rows: &'rows [Value],
    field: &Value,
) -> Result<Vec<&'rows Value>, Box<dyn Error>> {
    let ids = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("the addressed field's contributor IDs")?;
    Ok(rows
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .filter(|source| source.get("id").is_some_and(|id| ids.contains(id)))
        .collect())
}

fn assert_translation(
    rows: &[Value],
    field: &Value,
    definition: &Value,
    language: &str,
) -> TestResult {
    let english = source_id(
        rows,
        "archiveEntry",
        "/path/value",
        r"Data\Localization\english\global.ini",
    )?;
    let uses = field_uses(rows, field)?;
    let lookup = uses
        .iter()
        .find(|source| {
            source.get("source") == Some(english)
                && source.pointer("/location/type").and_then(Value::as_str)
                    == Some("localizationLookup")
        })
        .ok_or("the emitted label's lookup in the supplied Archive")?;
    assert_eq!(
        lookup.pointer("/location/language").and_then(Value::as_str),
        Some(language),
        "the emitted label uses the requested language"
    );
    let selector = lookup
        .pointer("/location/selector")
        .ok_or("the ID of the original lookup key")?;
    assert!(
        uses.iter().any(|source| {
            source.get("id") == Some(selector) && source.get("source") == Some(definition)
        }),
        "the label lookup uses the key from this export's DataForge source"
    );
    assert!(
        uses.iter().any(|source| {
            source.get("source") == Some(english)
                && source
                    .pointer("/location/namespace")
                    .and_then(Value::as_str)
                    == Some("globalIniBytes")
        }),
        "the label cites the actual stored translation"
    );
    Ok(())
}
