// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_media::{Index, MediaError};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::build_origin_fixture::{self as fixture, Channel, Run, TestResult};

#[derive(Clone, Copy)]
enum Mutation {
    Hash,
    Ordinal,
    Path,
    Count,
    Eof,
}

pub(super) fn unknown_build_member(channel: &Channel, run: &Run) -> TestResult {
    let path = run.output.join("media-index.json");
    let original = fs::read(&path)?;
    let modified = with_unknown_build_member(&original)?;
    fs::write(&path, &modified)?;
    let result = p4kx_media::export_with(
        &channel.archive,
        &run.output,
        &fixture::options(),
        &fixture::registry()?,
    );
    assert!(
        result
            .as_ref()
            .err()
            .is_some_and(|failure| super::converted_artifact_support::contains(
                failure,
                |cause| matches!(cause, MediaError::Destination(_))
            )),
        "an unknown actual Build node cannot disappear in typed deserialization and receive a complete six-node reuse claim"
    );
    assert_eq!(
        fs::read(&path)?,
        modified,
        "a refused input Index stays untouched"
    );
    fs::write(path, original)?;
    Ok(())
}

pub(super) fn rebound_scopes(channel: &Channel) -> TestResult {
    for (case, mutation) in [
        ("hash", Mutation::Hash),
        ("ordinal", Mutation::Ordinal),
        ("path", Mutation::Path),
        ("count", Mutation::Count),
        ("eof", Mutation::Eof),
    ] {
        let mut run = channel.export(case, &fixture::options(), &fixture::registry()?)?;
        mutate(&mut run, mutation)?;
        rebind(&mut run)?;
        let path = run.output.join("media-index.json");
        let committed = fs::read(&path)?;
        let failure = p4kx_media::export_with(
            &channel.archive,
            &run.output,
            &fixture::options(),
            &fixture::registry()?,
        )
        .err()
        .ok_or("rebound invalid Index Build scope was reused")?;
        assert!(
            super::converted_artifact_support::contains(&failure, |cause| matches!(
                cause,
                MediaError::Destination(_)
            )),
            "{case} violates the actual declared Index Build scope: {failure}"
        );
        assert_eq!(
            fs::read(path)?,
            committed,
            "a rejected rebound {case} scope preserves the committed Index bytes"
        );
    }
    Ok(())
}

fn mutate(run: &mut Run, mutation: Mutation) -> TestResult {
    let mut touched = false;
    for row in &mut run.fields {
        touched |= mutate_row(row, mutation)?;
    }
    if matches!(mutation, Mutation::Count) {
        let receipt = run
            .index
            .field_origins
            .as_mut()
            .ok_or("actual origin receipts")?;
        for summary in &mut receipt.streams {
            if serde_json::to_value(summary.stream)?
                .get("type")
                .and_then(Value::as_str)
                == Some("indexBuild")
            {
                summary.fields = p4kx_extraction::DecimalU64::new(5);
            }
        }
    }
    assert!(
        touched,
        "tamper must alter its owning actual Index Build row"
    );
    Ok(())
}

fn mutate_row(row: &mut Value, mutation: Mutation) -> TestResult<bool> {
    let index_build =
        row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("indexBuild");
    match mutation {
        Mutation::Hash
            if index_build && row.get("type").and_then(Value::as_str) == Some("occurrence") =>
        {
            *row.get_mut("payload_sha256")
                .ok_or("actual occurrence hash")? = json!("0".repeat(64));
        }
        Mutation::Ordinal if index_build => {
            *row.pointer_mut("/scope/ordinal")
                .ok_or("actual scope ordinal")? = json!("1");
        }
        Mutation::Path
            if index_build
                && row.get("field").and_then(Value::as_str) == Some("/source/build/branch") =>
        {
            *row.get_mut("field").ok_or("actual field address")? = json!("/source/build/ghost");
        }
        Mutation::Count | Mutation::Eof
            if row.get("type").and_then(Value::as_str) == Some("eof") =>
        {
            return mutate_eof(row, mutation);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn mutate_eof(row: &mut Value, mutation: Mutation) -> TestResult<bool> {
    let streams = row
        .get_mut("streams")
        .and_then(Value::as_array_mut)
        .ok_or("actual EOF streams")?;
    if matches!(mutation, Mutation::Eof) {
        let before = streams.len();
        streams.retain(|summary| {
            summary.pointer("/stream/type").and_then(Value::as_str) != Some("indexBuild")
        });
        return Ok(before != streams.len());
    }
    let mut touched = false;
    for summary in streams.iter_mut().filter(|summary| {
        summary.pointer("/stream/type").and_then(Value::as_str) == Some("indexBuild")
    }) {
        *summary
            .get_mut("fields")
            .ok_or("actual stream field count")? = json!("5");
        touched = true;
    }
    Ok(touched)
}
fn rebind(run: &mut Run) -> TestResult {
    let mut fields = Vec::new();
    for row in &run.fields {
        serde_json::to_writer(&mut fields, row)?;
        fields.push(b'\n');
    }
    let receipt = run
        .index
        .field_origins
        .as_mut()
        .ok_or("actual origin receipts")?;
    fs::write(run.base.join(&receipt.fields.path), &fields)?;
    receipt.fields.size = u64::try_from(fields.len())?;
    receipt.fields.digest.hex = digest(&fields);
    run.index.generation.clear();
    run.index.artifact_base.clear();
    run.index.generation = canonical_generation(&run.index)?;
    run.index.artifact_base = format!("generations/{}", run.index.generation);
    let destination = run.output.join(&run.index.artifact_base);
    fs::rename(&run.base, &destination)?;
    run.base = destination;
    fs::write(
        run.output.join("media-index.json"),
        serde_json::to_vec_pretty(&run.index)?,
    )?;
    Ok(())
}

fn canonical_generation(index: &Index) -> TestResult<String> {
    Ok(digest(&serde_json::to_vec(index)?))
}

fn digest(bytes: &[u8]) -> String {
    p4kx_extraction::Checksum::Sha256(Sha256::digest(bytes).into()).hex()
}

pub(super) fn historical_missing_scope(channel: &Channel) -> TestResult {
    let mut run = channel.export("historical", &fixture::options(), &fixture::registry()?)?;
    erase_index_build_scope(&mut run)?;
    rebind(&mut run)?;
    let path = run.output.join("media-index.json");
    fs::write(&path, with_unknown_build_member(&fs::read(&path)?)?)?;
    let outcome = p4kx_media::export_with(
        &channel.archive,
        &run.output,
        &fixture::options(),
        &fixture::registry()?,
    )?;
    match outcome {
        p4kx_media::ExportOutcome::Skipped { index } => {
            let receipts = serde_json::to_value(index.field_origins)?;
            assert!(
                !receipts
                    .get("streams")
                    .and_then(Value::as_array)
                    .ok_or("historical summaries")?
                    .iter()
                    .any(
                        |summary| summary.pointer("/stream/type").and_then(Value::as_str)
                            == Some("indexBuild")
                    ),
                "historical absence stays unknown rather than certifying a missing scope"
            );
        }
        p4kx_media::ExportOutcome::Published { index, .. } => {
            let receipts = serde_json::to_value(index.field_origins)?;
            assert!(
                receipts
                    .get("streams")
                    .and_then(Value::as_array)
                    .ok_or("upgraded summaries")?
                    .iter()
                    .any(
                        |summary| summary.pointer("/stream/type").and_then(Value::as_str)
                            == Some("indexBuild")
                    ),
                "regeneration must emit the current applicable carrier"
            );
        }
        _ => return Err("historical scope absence became an incomplete generation".into()),
    }
    Ok(())
}

fn with_unknown_build_member(original: &[u8]) -> TestResult<Vec<u8>> {
    let mut index: Value = serde_json::from_slice(original)?;
    let build = index
        .pointer_mut("/source/build")
        .and_then(Value::as_object_mut)
        .ok_or("actual Build object")?;
    build.insert("unknown_source_datum".into(), json!({"retained": "extra"}));
    Ok(serde_json::to_vec(&index)?)
}

fn erase_index_build_scope(run: &mut Run) -> TestResult {
    run.fields.retain(|row| {
        row.pointer("/scope/stream/type").and_then(Value::as_str) != Some("indexBuild")
    });
    for row in &mut run.fields {
        if row.get("type").and_then(Value::as_str) == Some("eof") {
            assert!(
                mutate_eof(row, Mutation::Eof)?,
                "the genuine writer declared its new scope before historical erasure"
            );
        }
    }
    let receipts = run
        .index
        .field_origins
        .as_mut()
        .ok_or("actual origin receipts")?;
    let mut retained = Vec::new();
    for summary in std::mem::take(&mut receipts.streams) {
        if serde_json::to_value(summary.stream)?
            .get("type")
            .and_then(Value::as_str)
            != Some("indexBuild")
        {
            retained.push(summary);
        }
    }
    receipts.streams = retained;
    Ok(())
}
