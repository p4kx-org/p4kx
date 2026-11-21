// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "../../../p4kx/tests/it/catalog_snapshot_inventory.rs"]
mod inventory;

use std::fs;
use std::path::Path;
use std::ptr;

use p4kx_export::CatalogPublicationMode;
use p4kx_gui::{CatalogExportOptions, CatalogExportStatus, Model};
use serde_json::Value;

use crate::cataloging::{self, TestResult};

#[test]
fn finished_catalog_inventory_is_the_summary_snapshot_and_the_written_provenance() -> TestResult {
    for (builder, mode) in [
        (cataloging::makers(), CatalogPublicationMode::Normal),
        (
            cataloging::unreadable(),
            CatalogPublicationMode::DiagnosticPartial,
        ),
    ] {
        let (_dir, mut model, out) = cataloging::opened(&builder)?;
        let file = out.join("snapshot.json");
        assert!(
            model.start_catalog_with_options(
                file.clone(),
                CatalogExportOptions {
                    publication_mode: mode,
                    ..CatalogExportOptions::default()
                },
                || {}
            ),
            "the actual background job starts"
        );
        cataloging::ended(&mut model)?;
        assert_finished(&model, &file)?;
        model.close_catalog();
        assert!(
            model.catalog().is_none(),
            "closing removes the result and its accessor"
        );
    }
    Ok(())
}

#[test]
fn incomplete_catalog_keeps_its_original_inventory_without_publishing() -> TestResult {
    let (_dir, mut model, out) = cataloging::opened(&cataloging::unreadable())?;
    let file = out.join("refused.json");
    fs::write(&file, "previous verified generation")?;
    assert!(
        model.start_catalog(file.clone(), || {}),
        "the actual normal job starts"
    );
    cataloging::ended(&mut model)?;
    assert_refusal_inventory(&model)?;
    assert_eq!(
        fs::read(&file)?.as_slice(),
        b"previous verified generation",
        "refusal preserves bytes"
    );
    assert_eq!(
        cataloging::files_in(&out)?,
        ["refused.json"],
        "no unpublished temporary artifact"
    );
    model.close_catalog();
    assert!(
        model.catalog().is_none(),
        "closed refusal exposes no result"
    );
    Ok(())
}

#[test]
fn running_cancelled_and_closed_jobs_expose_no_completed_inventory() -> TestResult {
    let (_dir, mut model, out) = cataloging::opened(&cataloging::makers())?;
    assert!(model.catalog().is_none(), "no result before starting");
    let hold = model.hold_catalog();
    let file = out.join("cancelled.json");
    fs::write(&file, "previous verified generation")?;
    assert!(
        model.start_catalog(file.clone(), || {}),
        "the actual held job starts"
    );
    cataloging::building(&mut model)?;
    assert_no_inventory(&model)?;
    model.cancel_catalog();
    assert!(
        matches!(
            cataloging::ended(&mut model)?,
            CatalogExportStatus::Cancelled
        ),
        "actual cancellation"
    );
    drop(hold);
    assert_no_inventory(&model)?;
    assert_eq!(
        fs::read(&file)?.as_slice(),
        b"previous verified generation",
        "cancel preserves bytes"
    );
    assert_eq!(
        cataloging::files_in(&out)?,
        ["cancelled.json"],
        "no partial publication"
    );
    model.close_catalog();
    assert!(
        model.catalog().is_none(),
        "closed cancellation exposes no result"
    );
    Ok(())
}

#[test]
fn unrelated_refusals_and_io_failure_do_not_expose_a_previous_finished_snapshot() -> TestResult {
    let (dir, mut model, out) = cataloging::opened(&cataloging::makers())?;
    assert!(
        model.start_catalog(out.join("first.json"), || {}),
        "the actual first job starts"
    );
    assert!(
        matches!(
            cataloging::ended(&mut model)?,
            CatalogExportStatus::Finished(_)
        ),
        "actual first result"
    );
    assert!(
        model
            .catalog()
            .and_then(|run| run.source_inventory())
            .is_some(),
        "a real preceding snapshot"
    );
    assert_target_refusals(&mut model, dir.path(), &out)?;
    assert_output_failure(&mut model, &out)?;
    Ok(())
}

fn assert_no_inventory(model: &Model) -> TestResult {
    let run = model.catalog().ok_or("actual noncatalog-result state")?;
    assert!(
        run.source_inventory().is_none(),
        "unrelated states cannot retain an earlier snapshot"
    );
    Ok(())
}

fn assert_finished(model: &Model, file: &Path) -> TestResult {
    let run = model.catalog().ok_or("actual finished job")?;
    let CatalogExportStatus::Finished(outcome) = run.status() else {
        return Err("actual finished catalog outcome".into());
    };
    let snapshot = run.source_inventory().ok_or("finished borrowed snapshot")?;
    assert!(
        ptr::eq(snapshot, outcome.summary().source_inventory()),
        "the GUI borrows its own Summary allocation without a second snapshot"
    );
    let document: Value = serde_json::from_slice(&fs::read(file)?)?;
    let serialized = serde_json::to_value(snapshot)?;
    assert_eq!(
        document.get("provenance"),
        Some(&serialized),
        "actual file pairing"
    );
    assert_eq!(
        document.get("exportHealth"),
        Some(&serde_json::to_value(outcome.summary().export_health())?),
        "actual report pairing"
    );
    let guid = document
        .pointer("/definitions/manufacturers/rows/0/guid")
        .ok_or("actual retained Manufacturer GUID")?;
    assert!(
        inventory::check(guid, &serialized)? > 0,
        "a published identity has its own contributing references"
    );
    assert!(
        inventory::check(&document, &serialized)? > 0,
        "actual published datum references resolve in this result's snapshot"
    );
    Ok(())
}

fn assert_refusal_inventory(model: &Model) -> TestResult {
    let run = model.catalog().ok_or("actual refusal job")?;
    let CatalogExportStatus::Incomplete(health) = run.status() else {
        return Err("actual normal incomplete refusal".into());
    };
    assert!(
        !health.is_complete(),
        "the retained report has an actual selected source failure"
    );
    assert!(
        health.failed_records().iter().any(|record| record
            .guid()
            .value()
            .is_some_and(|guid| guid.as_str() == "90000000-0000-4000-8000-000000000009")),
        "the original failed owner"
    );
    let snapshot = run.source_inventory().ok_or("paired refusal inventory")?;
    let serialized = serde_json::to_value(snapshot)?;
    inventory::check(&serde_json::to_value(health)?, &serialized)?;
    assert!(
        ptr::eq(
            snapshot,
            run.source_inventory().ok_or("same refusal snapshot")?
        ),
        "repeated access borrows the same immutable result"
    );
    Ok(())
}

fn assert_target_refusals(model: &mut Model, dir: &Path, out: &Path) -> TestResult {
    assert!(
        !model.start_catalog(out.join("refused.gz"), || {}),
        "gzip target is refused"
    );
    assert!(
        matches!(
            model.catalog().ok_or("gzip refusal")?.status(),
            CatalogExportStatus::GzipNotSupported
        ),
        "actual unrelated gzip state"
    );
    assert_no_inventory(model)?;
    assert!(
        !model.start_catalog(dir.join("Data.p4k"), || {}),
        "Archive target is refused"
    );
    assert!(
        matches!(
            model.catalog().ok_or("Archive refusal")?.status(),
            CatalogExportStatus::IsArchive
        ),
        "actual unrelated Archive state"
    );
    assert_no_inventory(model)?;
    Ok(())
}

fn assert_output_failure(model: &mut Model, out: &Path) -> TestResult {
    let missing = out.join("missing").join("catalog.json");
    assert!(
        model.start_catalog(missing.clone(), || {}),
        "actual failing output job starts"
    );
    assert!(
        matches!(
            cataloging::ended(model)?,
            CatalogExportStatus::Failed(Some(_))
        ),
        "actual output failure"
    );
    assert_no_inventory(model)?;
    assert!(!missing.exists(), "failed output is not published");
    assert_eq!(
        cataloging::files_in(out)?,
        ["first.json"],
        "only the prior complete publication exists"
    );
    Ok(())
}
