// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;
use std::slice;

use p4kx_extraction::{ConflictPolicy, OriginDigestWriter};
use p4kx_gui::{Model, RecordExportStatus, RecordExportSummary, RecordForm};
use serde_json::Value;

use super::choose;
use crate::exporting::{TestResult, database, ended, opened, opened_with_key};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};

fn run<'model>(
    model: &'model mut Model,
    out: &Path,
    form: RecordForm,
    policy: ConflictPolicy,
) -> Result<&'model RecordExportSummary, Box<dyn Error>> {
    model.begin_record_export();
    choose(model, out, form)?;
    let setup = model.record_export_setup_mut().ok_or("export setup")?;
    setup.set_filter("weapons");
    setup.set_policy(policy);
    assert!(model.submit_record_export(|| {}), "Record export started");
    let RecordExportStatus::Finished(summary) = ended(model)? else {
        return Err("no Record summary".into());
    };
    Ok(summary)
}

fn proof(
    path: &Path,
    payload: &[u8],
    address_type: &str,
    field: &str,
    encrypted: bool,
) -> Result<(), Box<dyn Error>> {
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(payload)?;
    let digest = serde_json::to_value(measured.finish()?)?;
    let content = fs::read_to_string(path)?;
    let rows: Vec<Value> = content
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let header = rows.first().ok_or("companion header")?;
    assert_eq!(
        header.pointer("/binding/sha256"),
        digest.get("sha256"),
        "binding authenticates GUI export bytes"
    );
    assert_eq!(
        header.pointer("/binding/byteLength"),
        digest.get("byteLength"),
        "binding measures full GUI output"
    );
    let field_key = if address_type == "textBytes" {
        "/address/field"
    } else {
        "/address/path"
    };
    let selected = rows
        .iter()
        .find(|row| {
            row.pointer("/address/type").and_then(Value::as_str) == Some(address_type)
                && row.pointer(field_key).and_then(Value::as_str) == Some(field)
        })
        .ok_or("specific emitted GUI field")?;
    assert_eq!(
        selected
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(encrypted),
        "the marker reflects the stored DataForge Entry"
    );
    assert_eq!(
        selected.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "the field evidence is complete"
    );
    assert!(
        selected
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .is_some_and(|uses| !uses.is_empty()),
        "the field retains original uses"
    );
    Ok(())
}

#[test]
fn planned_gui_files_and_streams_publish_exact_fields_and_keep_the_written_count() -> TestResult {
    let (_dir, mut model, out) = opened(&database())?;
    for (form, file, address_type, field) in [
        (
            RecordForm::Json,
            "libs/foundry/records/weapons/AmmoParams.Ballistic_S1.json",
            "jsonField",
            "/$guid",
        ),
        (
            RecordForm::Ndjson,
            "records.ndjson",
            "ndjsonField",
            "/$guid",
        ),
        (
            RecordForm::FlatCsv,
            "values.csv",
            "textBytes",
            "/rows/0/record_guid",
        ),
        (
            RecordForm::FlatNdjson,
            "values.ndjson",
            "ndjsonField",
            "/record_guid",
        ),
    ] {
        let summary = run(&mut model, &out, form, ConflictPolicy::Skip)?;
        let companion = out.join(format!("{file}.p4kx-origin.jsonl"));
        assert_planned_origin_summary(summary, form, &companion)?;
        proof(
            &companion,
            &fs::read(out.join(file))?,
            address_type,
            field,
            false,
        )?;
    }
    Ok(())
}

fn assert_planned_origin_summary(
    summary: &RecordExportSummary,
    form: RecordForm,
    companion: &Path,
) -> TestResult {
    assert_eq!(summary.written(), 2, "{form}: both definitions");
    assert_eq!(
        summary.origin_receipts().len(),
        if form == RecordForm::Json { 2 } else { 1 },
        "{form}: one companion per committed file"
    );
    assert!(
        summary.origin_failures().is_empty(),
        "{form}: no companion failures"
    );
    let aggregate = summary
        .provenance()
        .ok_or("written count proof")?
        .map_err(ToString::to_string)?;
    assert_eq!(
        aggregate.written.contains_encrypted_source(),
        Some(false),
        "{form}: original written count proof remains"
    );
    assert!(
        summary
            .origin_receipts()
            .iter()
            .any(|path| path.as_path() == companion),
        "{form}: published companion exposed from summary"
    );
    Ok(())
}

#[test]
fn gui_keeps_previous_record_bytes_and_proof_under_skip_and_if_changed() -> TestResult {
    let (_dir, mut model, out) = opened(&database())?;
    let first = run(&mut model, &out, RecordForm::Ndjson, ConflictPolicy::Skip)?;
    let companion = first
        .origin_receipts()
        .first()
        .ok_or("first committed companion")?
        .clone();
    let bytes = fs::read(&companion)?;
    for policy in [ConflictPolicy::Skip, ConflictPolicy::IfChanged] {
        let summary = run(&mut model, &out, RecordForm::Ndjson, policy)?;
        assert_eq!(
            summary.skipped(),
            ["records.ndjson"],
            "{policy:?}: file kept"
        );
        assert_eq!(
            summary.written(),
            0,
            "{policy:?}: kept file contributes no new definitions"
        );
        assert!(
            summary.origin_receipts().is_empty(),
            "{policy:?}: no new claims"
        );
        assert!(
            summary.origin_failures().is_empty(),
            "{policy:?}: no proof replacement attempted"
        );
        assert_eq!(
            fs::read(&companion)?,
            bytes,
            "{policy:?}: existing proof retained"
        );
    }
    Ok(())
}

#[test]
fn gui_reports_foreign_origin_file_without_discarding_committed_records() -> TestResult {
    let (_dir, mut model, out) = opened(&database())?;
    let companion = out.join("records.ndjson.p4kx-origin.jsonl");
    fs::write(&companion, b"foreign proof path")?;
    let summary = run(&mut model, &out, RecordForm::Ndjson, ConflictPolicy::Skip)?;
    assert_eq!(summary.written(), 2, "committed Record count remains");
    assert!(summary.failures().is_empty(), "neither Record failed");
    assert!(
        summary.error().is_none(),
        "the Record output itself committed"
    );
    assert_eq!(
        summary.origin_failures().len(),
        1,
        "companion failure remains visible"
    );
    assert!(
        summary.origin_receipts().is_empty(),
        "foreign proof is not reported as written"
    );
    assert!(
        matches!(summary.provenance(), Some(Ok(_))),
        "committed definition proof remains accessible"
    );
    assert_eq!(
        fs::read_to_string(out.join("records.ndjson"))?
            .lines()
            .count(),
        2,
        "both output lines remain readable"
    );
    assert_eq!(
        fs::read(&companion)?,
        b"foreign proof path",
        "foreign bytes stay untouched"
    );
    Ok(())
}

#[test]
fn gui_replaces_an_identical_payload_with_evidence_for_the_new_encrypted_entry() -> TestResult {
    let (_plain, mut model, out) = opened(&database())?;
    let first = run(&mut model, &out, RecordForm::Ndjson, ConflictPolicy::Skip)?;
    let prior_path = first
        .origin_receipts()
        .first()
        .ok_or("plain companion")?
        .clone();
    let prior = fs::read(&prior_path)?;
    let file = out.join("records.ndjson");
    let bytes = fs::read(&file)?;

    let (_encrypted, mut model, _) = opened_with_key(
        &ArchiveBuilder::new()
            .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted()),
        Some(test_key()),
    )?;
    let replaced = run(
        &mut model,
        &out,
        RecordForm::Ndjson,
        ConflictPolicy::Overwrite,
    )?;
    assert_eq!(replaced.written(), 2, "both selected Records were replaced");
    assert_eq!(
        replaced.origin_receipts(),
        slice::from_ref(&prior_path),
        "same companion path"
    );
    assert_eq!(fs::read(&file)?, bytes, "the physical output bytes match");
    assert_ne!(
        fs::read(&prior_path)?,
        prior,
        "the prior companion has been retired"
    );
    proof(&prior_path, &bytes, "ndjsonField", "/$guid", true)?;
    Ok(())
}
