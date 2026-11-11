// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_extraction::{
    DecimalU64, FieldPath, InputScope, OriginArtifactBinding, OriginArtifactName,
    OriginCompanionRow, OriginDigestWriter, OriginOutputAddress, SourceInput, SourceLocation,
    SourceUseRole, SourceUses, read_observed_file, write_origin_companion,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn checked_in_standalone_origin_schema_matches_the_typed_rows() -> TestResult {
    let schema = schemars::schema_for!(OriginCompanionRow<'static>).to_value();
    let generated = serde_json::to_string_pretty(&schema)? + "\n";
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/p4kx-source-origin.schema.json");
    if std::env::var_os("P4KX_BLESS").is_some_and(|value| value == "1") {
        fs::write(&path, &generated)?;
    }
    assert_eq!(
        fs::read_to_string(&path)?,
        generated,
        "{} is stale",
        path.display()
    );
    jsonschema::validator_for(&schema)?;
    Ok(())
}

#[test]
fn emitted_origin_rows_and_binding_validate_with_required_markers() -> TestResult {
    let schema = schemars::schema_for!(OriginCompanionRow<'static>).to_value();
    let validator = jsonschema::validator_for(&schema)?;
    let rows = emitted_fixture_rows()?;
    assert_eq!(rows.first().ok_or("header")?["binding"]["byteLength"], "3");
    assert_eq!(
        rows.first().ok_or("header")?["binding"]["sha256"],
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    for row in &rows {
        validator
            .validate(row)
            .map_err(|error| format!("{error}: {row}"))?;
    }
    let header = rows.first().ok_or("header")?;
    let mut wrong = header.clone();
    wrong["formatVersion"] = json!("2.0");
    assert!(!validator.is_valid(&wrong));
    wrong = header.clone();
    wrong["binding"]["byteLength"] = json!(3);
    assert!(!validator.is_valid(&wrong));
    let field = rows
        .iter()
        .find(|row| row["type"] == "field")
        .ok_or("field")?;
    assert_eq!(field["containsEncryptedSource"], Value::Null);
    assert_eq!(field["sourceSetComplete"], true);
    let mut wrong = field.clone();
    wrong
        .as_object_mut()
        .ok_or("field object")?
        .remove("containsEncryptedSource");
    assert!(
        !validator.is_valid(&wrong),
        "each datum must expose its marker: {wrong}"
    );
    let mut wrong = field.clone();
    wrong["sourceUsesRefs"] = json!(["unknown", "unknown"]);
    assert!(
        !validator.is_valid(&wrong),
        "duplicates are invalid: {wrong}"
    );
    let mut wrong = field.clone();
    wrong["containsEncryptedSource"] = json!(false);
    wrong["sourceSetComplete"] = json!(false);
    assert!(
        !validator.is_valid(&wrong),
        "clear requires complete coverage: {wrong}"
    );
    Ok(())
}

fn emitted_fixture_rows() -> Result<Vec<Value>, Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("origin.txt");
    fs::write(&source, b"abc")?;
    let input = read_observed_file(&source)?;
    let uses = SourceUses::new();
    let observed = uses.observe(
        SourceInput::File(input.origin().ok_or("missing source")?),
        InputScope::Payload,
    )?;
    let datum = observed.at(
        SourceLocation::Subresource {
            namespace: "fixture".to_owned(),
            indices: Vec::new(),
            field: "value".to_owned(),
        },
        SourceUseRole::Conversion,
    )?;
    let origin = uses.origin(&[datum], true);
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(input.bytes())?;
    let binding = OriginArtifactBinding::File {
        path: OriginArtifactName::from_basename(OsStr::new("result.json"))?,
        digest: measured.finish()?,
    };
    let address = OriginOutputAddress::JsonField {
        path: FieldPath::parse("/a~1b/0")?,
    };
    let mut bytes = Vec::new();
    write_origin_companion(&mut bytes, &binding, &uses.freeze()?, [(&address, &origin)])?;
    Ok(bytes
        .split(|&byte| byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(serde_json::from_slice)
        .collect::<Result<Vec<Value>, _>>()?)
}

#[test]
fn text_fields_require_exact_decimal_ranges_and_a_logical_field() -> TestResult {
    let schema = schemars::schema_for!(OriginCompanionRow<'static>).to_value();
    let validator = jsonschema::validator_for(&schema)?;
    let uses = SourceUses::new();
    let origin = uses.origin(&[], false);
    let address = OriginOutputAddress::TextBytes {
        start: DecimalU64::new(2),
        end_exclusive: DecimalU64::new(4),
        field: FieldPath::parse("/changes/0/old")?,
    };
    let row = serde_json::to_value(OriginCompanionRow::Field {
        address: &address,
        origin: &origin,
    })?;
    assert_eq!(
        row["address"],
        json!({"type":"textBytes","start":"2","endExclusive":"4","field":"/changes/0/old"})
    );
    validator
        .validate(&row)
        .map_err(|error| error.to_string())?;
    let mut no_field = row.clone();
    no_field["address"]
        .as_object_mut()
        .ok_or("text address")?
        .remove("field");
    assert!(
        !validator.is_valid(&no_field),
        "a span needs its emitted logical field"
    );
    let mut numeric_start = row;
    numeric_start["address"]["start"] = json!(2);
    assert!(
        !validator.is_valid(&numeric_start),
        "offsets use exact decimal strings"
    );
    Ok(())
}
