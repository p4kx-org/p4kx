// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::origin_read_support::{Fixture, TestResult};
use crate::{Index, hashing};

#[test]
fn current_shared_uses_require_captured_effective_encryption() -> TestResult {
    let fixture = Fixture::new()?;
    let mut index = fixture.index.clone();
    let mut rows = rows(&fixture, true)?;
    rows.first_mut()
        .and_then(Value::as_object_mut)
        .ok_or("original shared use")?
        .remove("effectiveEncryption");
    rewrite(&fixture, &mut index, true, &rows)?;
    assert!(
        fixture.verify(&index).is_err(),
        "current receipt capability cannot substitute missing per-use evidence"
    );
    Ok(())
}

#[test]
fn nested_original_source_fact_references_are_closed() -> TestResult {
    let fixture = Fixture::new()?;
    let mut index = fixture.index.clone();
    let mut rows = rows(&fixture, false)?;
    let fact = rows
        .iter_mut()
        .find_map(|row| row.get_mut("path").and_then(Value::as_object_mut))
        .ok_or("full original shared path fact")?;
    fact.get_mut("sourceUsesRefs")
        .and_then(Value::as_array_mut)
        .ok_or("original fact origin")?
        .push(Value::String("missing-original-fact-use".into()));
    rewrite(&fixture, &mut index, false, &rows)?;
    assert!(
        fixture.verify(&index).is_err(),
        "closure includes nested SourcedValue metadata inside the original source inventory"
    );
    Ok(())
}

#[test]
fn attempted_and_reference_uses_cannot_supply_field_origins() -> TestResult {
    for role in ["attempted", "reference"] {
        let fixture = Fixture::new()?;
        let mut index = fixture.index.clone();
        let mut rows = rows(&fixture, true)?;
        let reference = field_reference(&fixture)?;
        let target = rows
            .iter_mut()
            .find(|row| row.get("id").and_then(Value::as_str) == Some(reference.as_str()))
            .and_then(Value::as_object_mut)
            .ok_or("actual contributing field use")?;
        assert_ne!(
            target.get("role").and_then(Value::as_str),
            Some(role),
            "the mutation changes an actual contributing field use"
        );
        target.insert("role".into(), Value::String(role.into()));
        rewrite(&fixture, &mut index, true, &rows)?;
        assert!(
            fixture.verify(&index).is_err(),
            "{role} stays visible in its inventory without tainting a data value"
        );
    }
    Ok(())
}

#[test]
fn original_ancestor_and_use_source_references_are_closed() -> TestResult {
    for uses in [false, true] {
        let fixture = Fixture::new()?;
        let mut index = fixture.index.clone();
        let mut rows = rows(&fixture, uses)?;
        let row = rows
            .first_mut()
            .and_then(Value::as_object_mut)
            .ok_or("original inventory row")?;
        if uses {
            row.insert(
                "source".into(),
                Value::String("missing-physical-source".into()),
            );
        } else {
            row.insert(
                "ancestors".into(),
                serde_json::json!(["missing-physical-ancestor"]),
            );
        }
        rewrite(&fixture, &mut index, uses, &rows)?;
        assert!(
            fixture.verify(&index).is_err(),
            "all original inventory edges are closed"
        );
    }
    Ok(())
}

fn field_reference(fixture: &Fixture) -> Result<String, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(fixture.root.join(&fixture.receipts().fields.path))?;
    for line in text.lines() {
        let row: Value = serde_json::from_str(line)?;
        if let Some(reference) = row
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .and_then(|references| references.first())
            .and_then(Value::as_str)
        {
            return Ok(reference.to_owned());
        }
    }
    Err("the actual fixture has a contributing field use".into())
}
fn rows(fixture: &Fixture, uses: bool) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let artifact = if uses {
        &fixture.receipts().uses
    } else {
        &fixture.receipts().sources
    };
    let text = fs::read_to_string(fixture.root.join(&artifact.path))?;
    text.lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn rewrite(fixture: &Fixture, index: &mut Index, uses: bool, rows: &[Value]) -> TestResult {
    let receipts = index.field_origins.as_mut().ok_or("current group")?;
    let artifact = if uses {
        &mut receipts.uses
    } else {
        &mut receipts.sources
    };
    let mut bytes = Vec::new();
    for row in rows {
        serde_json::to_writer(&mut bytes, row)?;
        bytes.push(b'\n');
    }
    fs::write(fixture.root.join(&artifact.path), &bytes)?;
    artifact.size = u64::try_from(bytes.len())?;
    artifact.digest.hex = hashing::hex(Sha256::digest(&bytes).into());
    Ok(())
}

#[test]
fn minimal_source_proof_requires_canonical_index_and_capture_limited_details() -> TestResult {
    for mutation in [
        ProofMutation::MissingIndex,
        ProofMutation::LeadingZero,
        ProofMutation::NumericIndex,
        ProofMutation::Overflow,
        ProofMutation::MissingDetails,
        ProofMutation::FullDetails,
    ] {
        let fixture = Fixture::new()?;
        let mut index = fixture.index.clone();
        let mut source_rows = proof_rows(&fixture)?;
        rewrite(&fixture, &mut index, false, &source_rows)?;
        assert!(
            fixture.verify(&index).is_ok(),
            "valid original minimal description before {mutation:?}"
        );
        let proof = source_rows
            .iter_mut()
            .find(|row| row.get("kind").and_then(Value::as_str) == Some("archiveEntryProof"))
            .and_then(Value::as_object_mut)
            .ok_or("minimal proof")?;
        mutate_proof(mutation, proof);
        rewrite(&fixture, &mut index, false, &source_rows)?;
        assert!(
            fixture.verify(&index).is_err(),
            "{mutation:?} is not a current original proof description"
        );
    }
    Ok(())
}

fn mutate_proof(mutation: ProofMutation, proof: &mut serde_json::Map<String, Value>) {
    match mutation {
        ProofMutation::MissingIndex => {
            proof.remove("entryIndex");
        }
        ProofMutation::LeadingZero => {
            proof.insert("entryIndex".into(), Value::String("00".into()));
        }
        ProofMutation::NumericIndex => {
            proof.insert("entryIndex".into(), Value::from(0));
        }
        ProofMutation::Overflow => {
            proof.insert(
                "entryIndex".into(),
                Value::String("18446744073709551616".into()),
            );
        }
        ProofMutation::MissingDetails => {
            proof.remove("details");
        }
        ProofMutation::FullDetails => {
            proof.insert("details".into(), Value::String("full".into()));
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ProofMutation {
    MissingIndex,
    LeadingZero,
    NumericIndex,
    Overflow,
    MissingDetails,
    FullDetails,
}

#[test]
fn full_original_source_keeps_its_required_typed_description() -> TestResult {
    for field in [
        "entryIndex",
        "path",
        "recordFormat",
        "recordRange",
        "zipFlags",
        "declarations",
    ] {
        let fixture = Fixture::new()?;
        let mut index = fixture.index.clone();
        let mut source_rows = rows(&fixture, false)?;
        let source = source_rows
            .iter_mut()
            .find(|row| row.get("kind").and_then(Value::as_str) == Some("archiveEntry"))
            .and_then(Value::as_object_mut)
            .ok_or("full original source")?;
        source.remove(field);
        rewrite(&fixture, &mut index, false, &source_rows)?;
        assert!(
            fixture.verify(&index).is_err(),
            "a rehashed current full description cannot omit required {field}"
        );
    }
    Ok(())
}

#[test]
fn full_original_source_keeps_decimal_range_and_numeric_index_forms() -> TestResult {
    for field in ["entryIndex", "rangeOffset"] {
        let fixture = Fixture::new()?;
        let mut index = fixture.index.clone();
        let mut source_rows = rows(&fixture, false)?;
        let source = source_rows
            .iter_mut()
            .find(|row| row.get("kind").and_then(Value::as_str) == Some("archiveEntry"))
            .ok_or("full original source")?;
        let value = if field == "entryIndex" {
            source.get_mut("entryIndex").ok_or("full numeric index")?
        } else {
            source
                .pointer_mut("/recordRange/offset")
                .ok_or("range offset")?
        };
        *value = if field == "entryIndex" {
            Value::String("0".into())
        } else {
            Value::from(0)
        };
        rewrite(&fixture, &mut index, false, &source_rows)?;
        assert!(
            fixture.verify(&index).is_err(),
            "{field} retains its original typed wire form"
        );
    }
    Ok(())
}

fn proof_rows(fixture: &Fixture) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut source_rows = rows(fixture, false)?;
    let source = source_rows
        .iter_mut()
        .find(|row| row.get("kind").and_then(Value::as_str) == Some("archiveEntry"))
        .and_then(Value::as_object_mut)
        .ok_or("full original source")?;
    let index = source
        .get("entryIndex")
        .and_then(Value::as_u64)
        .ok_or("numeric source index")?;
    source.retain(|key, _| matches!(key.as_str(), "id" | "ancestors" | "storedEncryption"));
    source.insert("kind".into(), Value::String("archiveEntryProof".into()));
    source.insert("entryIndex".into(), Value::String(index.to_string()));
    source.insert("details".into(), Value::String("captureLimited".into()));
    Ok(source_rows)
}
#[test]
fn original_source_fact_requires_explicit_nullable_value_and_marker_keys() -> TestResult {
    for field in ["value", "containsEncryptedSource"] {
        let fixture = Fixture::new()?;
        let mut index = fixture.index.clone();
        let mut source_rows = rows(&fixture, false)?;
        let fact = source_rows
            .iter_mut()
            .find_map(|row| row.get_mut("path").and_then(Value::as_object_mut))
            .ok_or("original path fact")?;
        fact.remove(field);
        rewrite(&fixture, &mut index, false, &source_rows)?;
        assert!(
            fixture.verify(&index).is_err(),
            "required nullable fact key {field} cannot disappear"
        );
    }
    Ok(())
}

#[test]
fn original_use_coordinates_require_metadata_field_and_decimal_ordinals() -> TestResult {
    for subresource in [false, true] {
        let fixture = Fixture::new()?;
        let mut index = fixture.index.clone();
        let mut use_rows = rows(&fixture, true)?;
        let expected = if subresource {
            "subresource"
        } else {
            "metadata"
        };
        let location = use_rows
            .iter_mut()
            .find_map(|row| {
                let location = row.get_mut("location")?;
                (location.get("type").and_then(Value::as_str) == Some(expected))
                    .then(|| location.as_object_mut())
                    .flatten()
            })
            .ok_or("actual typed source coordinate")?;
        if subresource {
            location
                .get_mut("indices")
                .and_then(Value::as_array_mut)
                .ok_or("original ordinals")?
                .push(Value::String("00".into()));
        } else {
            location.remove("field");
        }
        rewrite(&fixture, &mut index, true, &use_rows)?;
        assert!(
            fixture.verify(&index).is_err(),
            "{expected} retains its required coordinate shape"
        );
    }
    Ok(())
}
#[path = "origin_physical_encryption_tests.rs"]
mod physical_encryption;
