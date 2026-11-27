// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use serde_json::Value;

use super::{TestResult, export, observed_uses, write_annotated_archive};

#[test]
fn compact_ndjson_binds_exact_tags_and_locale_on_the_same_line() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    write_annotated_archive(&archive)?;
    let output = directory.path().join("out");
    export(
        &archive,
        &output,
        &[
            "--format", "ndjson", "--filter", "kit", "--tags", "--lang", "en", "--json",
        ],
        0,
    )?;
    let file = output.join("records.ndjson");
    let payload: Value = serde_json::from_str(&fs::read_to_string(&file)?)?;
    assert_eq!(
        payload.pointer("/mount/tag").and_then(Value::as_str),
        Some("Weapon.Gun"),
        "the original chosen tag path is emitted"
    );
    assert_eq!(
        payload.pointer("/label/text").and_then(Value::as_str),
        Some("A test kit"),
        "the indexed language value is emitted"
    );

    let records = fs::read_to_string(file.with_file_name("records.ndjson.p4kx-origin.jsonl"))?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    for path in ["/mount/tag", "/label/text"] {
        let field = records
            .iter()
            .find(|row| row.pointer("/address/path").and_then(Value::as_str) == Some(path))
            .ok_or("selected annotated field")?;
        assert_eq!(
            field.pointer("/address/line").and_then(Value::as_str),
            Some("0"),
            "{path}"
        );
    }
    let tag = observed_uses(&records, "/mount/tag", true)?;
    for relation in ["children", "tags"] {
        assert!(
            tag.iter().any(|used| {
                used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeTagRelations")
                    && used.pointer("/location/field").and_then(Value::as_str) == Some(relation)
            }),
            "tag selection retains the original {relation} occurrence"
        );
    }
    let locale = observed_uses(&records, "/label/text", true)?;
    assert!(
        locale.iter().any(
            |used| used.pointer("/location/namespace").and_then(Value::as_str)
                == Some("globalIniBytes")
        ),
        "the translated value has its exact original"
    );
    Ok(())
}
