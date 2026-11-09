// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::origin_read_support::{self as support, Fixture, TestResult};
use crate::origin_verify::OriginAvailability;

#[test]
fn actual_archive_writer_roundtrips_all_hashed_origin_streams() -> TestResult {
    let fixture = Fixture::new()?;
    assert_eq!(
        fixture.verify(&fixture.index)?,
        OriginAvailability::Current,
        "actual Archive producer output verifies through the reuse reader"
    );
    let rows = fixture.rows()?;
    assert!(
        rows.iter()
            .any(|row| row.get("profile").and_then(Value::as_str) == Some("archiveEntry")),
        "production entry mapping is present"
    );
    assert_eq!(
        rows.last().and_then(support::row_type),
        Some("eof"),
        "exact terminal EOF"
    );
    Ok(())
}

#[test]
fn repeated_equal_artifacts_keep_distinct_scope_occurrences() -> TestResult {
    use sha2::{Digest as _, Sha256};

    let fixture = Fixture::new()?;
    let rows = fixture.rows()?;
    let manifest =
        std::fs::read_to_string(fixture.root.join(&fixture.index.artifact_manifest.path))?;
    let payloads: Vec<_> = manifest.split_inclusive('\n').collect();
    let artifacts: Vec<_> = rows
        .iter()
        .filter(|row| {
            support::row_type(row) == Some("occurrence")
                && row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("artifacts")
        })
        .collect();
    assert_eq!(
        artifacts.len(),
        payloads.len(),
        "every actual artifact occurrence is retained"
    );
    let digest = crate::hashing::hex(Sha256::digest(b"same decoded bytes").into());
    let mut repeated = Vec::new();
    for (ordinal, payload) in payloads.iter().enumerate() {
        let artifact: crate::Artifact = serde_json::from_str(payload)?;
        if artifact.digest.hex == digest {
            repeated.push((ordinal, *payload));
        }
    }
    assert_eq!(
        repeated.len(),
        2,
        "both authored equal payloads have actual manifest occurrences"
    );
    assert_eq!(
        repeated.first().map(|(_, line)| line),
        repeated.get(1).map(|(_, line)| line),
        "content dedup leaves byte-identical manifest rows"
    );
    let mut origins = Vec::new();
    for (ordinal, payload) in &repeated {
        origins.push(artifact_uses(&rows, &artifacts, *ordinal, payload)?);
    }
    assert_ne!(
        origins.first(),
        origins.get(1),
        "equal bytes retain distinct original physical source uses"
    );
    assert_eq!(
        fixture.verify(&fixture.index)?,
        OriginAvailability::Current,
        "positive control"
    );
    Ok(())
}

fn artifact_uses<'a>(
    rows: &'a [Value],
    occurrences: &[&Value],
    ordinal: usize,
    payload: &str,
) -> Result<&'a [Value], Box<dyn std::error::Error>> {
    use sha2::{Digest as _, Sha256};

    let ordinal = ordinal.to_string();
    let occurrence = occurrences
        .iter()
        .find(|row| artifact_scope(row, &ordinal))
        .ok_or("the actual equal-artifact occurrence is missing")?;
    let expected = crate::hashing::hex(Sha256::digest(payload.as_bytes()).into());
    assert_eq!(
        occurrence.get("payload_sha256").and_then(Value::as_str),
        Some(expected.as_str()),
        "the occurrence binds the actual original manifest line including its newline"
    );
    let field = rows
        .iter()
        .find(|row| {
            support::row_type(row) == Some("field")
                && artifact_scope(row, &ordinal)
                && row.get("field").and_then(Value::as_str) == Some("/digest/hex")
        })
        .ok_or("the actual artifact digest origin is missing")?;
    let references = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("the artifact digest contribution references are missing")?;
    assert!(
        !references.is_empty(),
        "the actual retained input contributes to its digest"
    );
    Ok(references)
}

fn artifact_scope(row: &Value, ordinal: &str) -> bool {
    row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("artifacts")
        && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal)
}

#[test]
fn old_missing_origin_group_is_unknown() -> TestResult {
    let fixture = Fixture::new()?;
    let mut historical = fixture.index.clone();
    historical.field_origins = None;
    assert_eq!(
        fixture.verify(&historical)?,
        OriginAvailability::HistoricalUnknown,
        "historical absence cannot claim the current migration capability"
    );
    Ok(())
}

#[test]
fn origin_object_order_and_additive_metadata_do_not_change_meaning() -> TestResult {
    let fixture = Fixture::new()?;
    let mut rows = fixture.rows()?;
    for row in &mut rows {
        row.as_object_mut().ok_or("origin object")?.insert(
            "futureMetadata".into(),
            serde_json::json!({"retainedMeaning":true}),
        );
    }
    let mut index = fixture.index.clone();
    fixture.rewrite(&mut index, &rows)?;
    assert_eq!(
        fixture.verify(&index)?,
        OriginAvailability::Current,
        "sorted JSON object keys and additive metadata preserve the transport"
    );
    Ok(())
}

#[test]
fn source_use_receipt_corruption_is_rejected() -> TestResult {
    let fixture = Fixture::new()?;
    assert_eq!(
        fixture.verify(&fixture.index)?,
        OriginAvailability::Current,
        "positive control"
    );
    let path = fixture.root.join(&fixture.receipts().uses.path);
    let mut bytes = std::fs::read(&path)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes)?;
    assert!(
        fixture.verify(&fixture.index).is_err(),
        "actual receipt hash/length closes the stream"
    );
    Ok(())
}

#[test]
fn missing_actual_artifact_occurrence_is_rejected() -> TestResult {
    let fixture = Fixture::new()?;
    let mut rows = fixture.rows()?;
    let start = rows
        .iter()
        .position(|row| {
            support::row_type(row) == Some("occurrence")
                && row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("artifacts")
                && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some("1")
        })
        .ok_or("second actual artifact occurrence")?;
    let end = rows
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, row)| (support::row_type(row) == Some("end")).then_some(index))
        .ok_or("matching end")?;
    rows.drain(start..=end);
    let mut index = fixture.index.clone();
    fixture.rewrite(&mut index, &rows)?;
    assert!(
        fixture.verify(&index).is_err(),
        "raw artifact dedup cannot collapse occurrence coverage"
    );
    Ok(())
}
