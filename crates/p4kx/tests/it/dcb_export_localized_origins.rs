// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use serde_json::Value;

use super::{TestResult, with_exported};

#[test]
fn localized_json_cites_the_actual_value_and_the_original_lookup_key() -> TestResult {
    for (language, path, byte_length, encrypted) in [
        (
            "en",
            "/description/text",
            Some("Rolls.\\nSlowly.".len()),
            false,
        ),
        ("en", "/labels/1/missing", None, false),
        ("de", "/description/missing", Some(0), true),
        ("de", "/labels/1/missing", None, true),
    ] {
        with_exported(&["--format", "json", "--lang", language], |directory| {
            let input = directory.join("libs/foundry/records/vehicles/cart.json.p4kx-origin.jsonl");
            let rows = fs::read_to_string(input)?
                .lines()
                .map(serde_json::from_str::<Value>)
                .collect::<Result<Vec<_>, _>>()?;
            let field = rows
                .iter()
                .find(|row| row.pointer("/address/path").and_then(Value::as_str) == Some(path))
                .ok_or("the annotation needs its own output address")?;
            assert_localized_use(&rows, field, language, byte_length, encrypted)
        })?;
    }
    Ok(())
}

fn assert_localized_use(
    rows: &[Value],
    field: &Value,
    language: &str,
    byte_length: Option<usize>,
    encrypted: bool,
) -> TestResult {
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&Value::Bool(true)),
        "complete authored key and language lookup"
    );
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&Value::Bool(encrypted)),
        "the selected language Entry determines the marker"
    );
    let ids = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let used: Vec<_> = rows
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .filter(|row| row.get("id").is_some_and(|id| ids.contains(id)))
        .collect();
    let lookup = used
        .iter()
        .find(|row| {
            row.pointer("/location/type").and_then(Value::as_str) == Some("localizationLookup")
        })
        .ok_or("the observed lookup must survive")?;
    assert_eq!(
        lookup.pointer("/location/language").and_then(Value::as_str),
        Some(language),
        "the selected language is retained"
    );
    let selector = lookup
        .pointer("/location/selector")
        .ok_or("the authored key reference")?;
    assert!(
        used.iter().any(|row| {
            row.get("id") == Some(selector)
                && row.get("role").and_then(Value::as_str) == Some("selector")
                && row.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeDecodedBytes")
                && row.pointer("/location/field").and_then(Value::as_str) == Some("text")
        }),
        "the lookup refers to the observed DataForge key bytes"
    );
    assert_original_value_range(&used, lookup, byte_length)
}

fn assert_original_value_range(
    used: &[&Value],
    lookup: &Value,
    byte_length: Option<usize>,
) -> TestResult {
    let value = used.iter().find(|row| {
        row.get("source") == lookup.get("source")
            && row.pointer("/location/namespace").and_then(Value::as_str) == Some("globalIniBytes")
    });
    assert_eq!(
        value.is_some(),
        byte_length.is_some(),
        "only stored values have byte ranges"
    );
    if let Some(value) = value {
        let measured = value
            .pointer("/location/indices/1")
            .and_then(Value::as_str)
            .ok_or("value byte length")?
            .parse::<usize>()?;
        assert_eq!(Some(measured), byte_length, "winning value byte count");
    }
    Ok(())
}

#[test]
fn localized_xml_binds_the_bytes_of_the_emitted_escaped_value() -> TestResult {
    with_exported(&["--lang", "en"], |directory| {
        let file = directory.join("libs/foundry/records/vehicles/cart.xml");
        let artifact = fs::read(&file)?;
        let rows = fs::read_to_string(file.with_file_name("cart.xml.p4kx-origin.jsonl"))?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        let source = "Rolls.\\nSlowly.";
        let field = rows
            .iter()
            .find(|row| {
                row.get("address").is_some_and(|address| {
                    address.get("type").and_then(Value::as_str) == Some("xmlBytes")
                        && address.get("field").and_then(Value::as_str) == Some("attributeValue")
                        && artifact_span(&artifact, address)
                            == Some(b"Rolls.&#10;Slowly.".as_slice())
                })
            })
            .ok_or("the escaped XML value has no exact byte address")?;
        assert_localized_use(&rows, field, "en", Some(source.len()), false)
    })
}

fn artifact_span<'data>(artifact: &'data [u8], address: &Value) -> Option<&'data [u8]> {
    let start = address.get("start")?.as_str()?.parse::<usize>().ok()?;
    let end = address
        .get("endExclusive")?
        .as_str()?
        .parse::<usize>()
        .ok()?;
    artifact.get(start..end)
}
