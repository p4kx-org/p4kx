// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Basetool catalog of the window driven headless: availability, the written file and its
//! summary, cancel, and the DataForge shared with the Record export.

use crate::cataloging;

use std::fs;
use std::io::Read;

use cataloging::{TestResult, building, ended, files_in, makers, many, opened, wait_until};
use p4kx_export::{Kind, Language as TextLanguage, SizeWarning};
use p4kx_gui::{Browser, CATALOG_FILE, CatalogExportStatus, Model};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

#[test]
fn an_unreadable_record_refuses_a_normal_catalog_without_writing() -> TestResult {
    let (_dir, mut model, out) = opened(&cataloging::unreadable())?;
    let file = out.join("basetool-catalog.json");

    assert!(model.start_catalog(file.clone(), || {}), "started");
    let CatalogExportStatus::Incomplete(health) = ended(&mut model)? else {
        return Err("no incomplete publication refusal".into());
    };
    assert!(!health.is_complete(), "a selected source failed");
    assert!(
        health
            .failed_records
            .iter()
            .any(|record| record.guid == "90000000-0000-4000-8000-000000000009"),
        "the failed source identity is retained"
    );
    assert!(!file.exists(), "normal publication creates no snapshot");
    Ok(())
}

#[test]
fn a_catalog_needs_an_archive_with_a_dataforge() -> TestResult {
    let (_with, with, _) = opened(&makers())?;
    let (_without, mut without, out) =
        opened(&ArchiveBuilder::new().entry(TestEntry::stored("readme.txt", "readme")))?;

    let started = without.start_catalog(out.join("catalog.json"), || {});

    assert!(with.catalog_available(), "offered with a DataForge");
    assert!(!without.catalog_available(), "not without one");
    assert!(!started, "nothing starts");
    assert!(without.catalog().is_none(), "no catalog");
    Ok(())
}

#[test]
fn the_catalog_is_written_and_summarised() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    let file = out.join("basetool-catalog.json");

    assert!(model.start_catalog(file.clone(), || {}), "started");
    let CatalogExportStatus::Finished(outcome) = ended(&mut model)? else {
        return Err("no summary".into());
    };
    let summary = outcome.summary();

    assert_eq!(outcome.file(), file, "the chosen file");
    let makers = summary
        .kind(Kind::Manufacturers)
        .ok_or("no manufacturers")?;
    assert_eq!(
        (makers.count, makers.skipped),
        (2, 1),
        "ACAS and DRAK, the leftover skipped"
    );
    assert!(makers.complete, "complete");
    let german = summary.text(TextLanguage::German).ok_or("no German")?;
    assert_eq!(german.unresolved, 1, "DRAK has no German name");
    assert_eq!(summary.size_warning(), None, "small");
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&file)?)?;
    assert_eq!(
        json["schemaVersion"],
        p4kx_export::SCHEMA_VERSION,
        "a Basetool catalog"
    );
    assert_eq!(
        json["definitions"]["manufacturers"]["rows"]
            .as_array()
            .map(Vec::len),
        Some(2),
        "two"
    );
    assert!(json.get("manufacturers").is_none(), "no legacy array");
    assert_eq!(
        json["definitions"]["manufacturers"]["availability"],
        "available"
    );
    let row = &json["definitions"]["manufacturers"]["rows"][0];
    for (field, domain) in [("name", "publicNames"), ("description", "gameTexts")] {
        let reference = &row["localization"]["value"][field];
        assert_eq!(reference["state"], "present");
        let texts = json["texts"][domain]
            .as_array()
            .ok_or("canonical text table")?;
        assert!(
            texts
                .iter()
                .any(|text| text["id"]["value"] == reference["value"])
        );
    }
    assert_eq!(
        files_in(&out)?,
        ["basetool-catalog.json"],
        "no temporary file"
    );
    Ok(())
}

#[test]
fn only_one_catalog_runs_at_a_time() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    let hold = model.hold_catalog();

    let first = model.start_catalog(out.join("first.json"), || {});
    building(&mut model)?;
    let second = model.start_catalog(out.join("second.json"), || {});
    let available = model.catalog_available();
    model.stop_catalog();
    drop(hold);

    assert!(first, "the first starts");
    assert!(!available, "not offered while one runs");
    assert!(!second, "the second does not start");
    assert!(!out.join("second.json").exists(), "nothing written for it");
    Ok(())
}

#[test]
fn cancelling_while_dataforge_is_read_ends_at_once_and_writes_nothing() -> TestResult {
    let (_dir, mut model, out) = opened(&many(20_000))?;
    let file = out.join("basetool-catalog.json");
    fs::write(&file, "old")?;

    model.start_catalog(file.clone(), || {});
    model.cancel_catalog();
    let ended_at_once = model.poll_catalog();
    let cancelled = matches!(ended(&mut model)?, CatalogExportStatus::Cancelled);
    wait_until(&mut model, Model::record_export_keeps_dataforge)?;

    assert!(ended_at_once, "the window does not wait for the read");
    assert!(cancelled, "the outcome says so");
    assert_eq!(fs::read_to_string(&file)?, "old", "the file is untouched");
    assert_eq!(
        files_in(&out)?,
        ["basetool-catalog.json"],
        "no partial file"
    );
    Ok(())
}

#[test]
fn cancelling_while_records_are_checked_writes_nothing() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    let hold = model.hold_catalog();

    model.start_catalog(out.join("basetool-catalog.json"), || {});
    building(&mut model)?;
    model.cancel_catalog();
    let status = ended(&mut model)?;
    drop(hold);

    assert!(
        matches!(status, CatalogExportStatus::Cancelled),
        "cancelled: {status:?}"
    );
    assert!(files_in(&out)?.is_empty(), "no file and no partial file");
    Ok(())
}

#[test]
fn the_dataforge_read_for_a_catalog_is_kept_for_the_record_export() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    let reads = |model: &Model| model.browser().map(Browser::dataforge_reads);
    let before = reads(&model);

    model.start_catalog(out.join("catalog.json"), || {});
    let finished = matches!(ended(&mut model)?, CatalogExportStatus::Finished(_));
    model.begin_record_export();
    let setup = model.record_export_setup_mut().ok_or("no setup")?;
    setup.set_filter("scitemmanufacturer");
    setup.set_kind(p4kx_gui::TargetKind::ExistingFolder);
    setup.set_folder(out.join("records"));
    let exported = model.submit_record_export(|| {});
    wait_until(&mut model, |model| {
        model.record_export().is_some_and(|run| !run.is_running())
    })?;

    assert_eq!(before, Some(0), "nothing read before");
    assert!(finished, "the catalog is written");
    assert!(exported, "the Record export starts");
    assert_eq!(reads(&model), Some(1), "DataForge read once for both");
    assert_eq!(
        files_in(&out.join("records/libs/foundry/records/scitemmanufacturer"))?.len(),
        3,
        "the Record export used it"
    );
    Ok(())
}

#[test]
fn a_catalog_into_a_missing_folder_fails_with_the_reason() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    let file = out.join("missing").join("catalog.json");

    model.start_catalog(file.clone(), || {});
    let CatalogExportStatus::Failed(Some(reason)) = ended(&mut model)? else {
        return Err("no failure".into());
    };
    let reason = reason.to_owned();

    assert!(!reason.is_empty(), "a reason: {reason}");
    assert!(!file.exists(), "nothing written");
    assert!(files_in(&out)?.is_empty(), "no partial file");
    Ok(())
}

#[test]
fn the_open_archive_is_refused_as_the_catalog_file() -> TestResult {
    let (dir, mut model, _) = opened(&makers())?;
    let archive = dir.path().join("Data.p4k");
    let before = fs::read(&archive)?;

    let started = model.start_catalog(archive.clone(), || {});

    assert!(!started, "nothing starts");
    assert!(
        model
            .catalog()
            .is_some_and(|run| matches!(run.status(), CatalogExportStatus::IsArchive)),
        "the outcome says why"
    );
    assert_eq!(fs::read(&archive)?, before, "the Archive is unchanged");
    Ok(())
}

#[test]
fn an_archive_change_during_a_catalog_fails_it_and_writes_nothing() -> TestResult {
    let (dir, mut model, out) = opened(&makers())?;
    let hold = model.hold_catalog();

    model.start_catalog(out.join("basetool-catalog.json"), || {});
    building(&mut model)?;
    cataloging::wordy(2, 10).write_to(dir.path().join("Data.p4k"))?;
    drop(hold);
    let status = ended(&mut model)?;

    assert!(
        matches!(status, CatalogExportStatus::Failed(Some(_))),
        "failed with the reason: {status:?}"
    );
    assert!(files_in(&out)?.is_empty(), "no file and no partial file");
    Ok(())
}

#[test]
fn a_gz_file_name_is_refused_and_nothing_is_written() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;

    let started = model.start_catalog(out.join("basetool-catalog.json.gz"), || {});

    assert!(!started, "nothing starts");
    assert!(
        model
            .catalog()
            .is_some_and(|run| matches!(run.status(), CatalogExportStatus::GzipNotSupported)),
        "the outcome says why"
    );
    assert!(files_in(&out)?.is_empty(), "no file");
    Ok(())
}

#[test]
fn the_file_name_of_the_dialog_is_normalised_before_the_catalog_is_written() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;

    model.start_catalog(out.join("typed.zst"), || {});
    ended(&mut model)?;
    model.start_catalog(out.join("doubled.json.json"), || {});
    ended(&mut model)?;

    assert_eq!(
        files_in(&out)?,
        ["doubled.json", "typed.json.zst"],
        "the names the catalog contract uses"
    );
    Ok(())
}

#[test]
fn a_zst_file_name_writes_the_catalog_as_a_zstd_frame() -> TestResult {
    let (_dir, mut model, out) = opened(&makers())?;
    let plain = out.join("plain.json");
    let packed = out.join("basetool-catalog.json.zst");

    model.start_catalog(plain.clone(), || {});
    ended(&mut model)?;
    model.start_catalog(packed.clone(), || {});
    let CatalogExportStatus::Finished(outcome) = ended(&mut model)? else {
        return Err("no summary".into());
    };

    let frame = fs::read(&packed)?;
    let mut decoded = Vec::new();
    zstd::Decoder::new(frame.as_slice())?.read_to_end(&mut decoded)?;
    let without_time = |bytes: &[u8]| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let mut json: serde_json::Value = serde_json::from_slice(bytes)?;
        let source = json["source"].as_object_mut().ok_or("no source")?;
        source.remove("exportedAt").ok_or("no export time")?;
        Ok(json)
    };
    assert!(
        without_time(&decoded)? == without_time(&fs::read(&plain)?)?,
        "the same catalog apart from the export time"
    );
    assert_eq!(
        outcome.summary().compressed_size(),
        Some(u64::try_from(frame.len())?),
        "the compressed size is the file size"
    );
    assert_eq!(
        files_in(&out)?,
        ["basetool-catalog.json.zst", "plain.json"],
        "no temporary file"
    );
    Ok(())
}

#[test]
fn a_catalog_near_the_upload_limit_is_flagged() -> TestResult {
    let (_dir, mut model, out) = opened(&cataloging::wordy(30, 1_000_000))?;

    model.start_catalog(out.join("catalog.json"), || {});
    let CatalogExportStatus::Finished(outcome) = ended(&mut model)? else {
        return Err("no summary".into());
    };

    assert_eq!(
        outcome.summary().size_warning(),
        Some(SizeWarning::NearLimit),
        "about 60 MB: {}",
        outcome.summary().size()
    );
    Ok(())
}

#[test]
fn the_suggested_catalog_file_is_the_compressed_one() {
    assert_eq!(CATALOG_FILE, "basetool-catalog.json.zst", "the upload form");
}

#[test]
fn an_independent_resource_has_its_own_written_kind_and_source_health() -> TestResult {
    let (_dir, mut model, out) = opened(&cataloging::resources())?;
    let file = out.join("resources.json");
    model.start_catalog(file.clone(), || {});
    let CatalogExportStatus::Finished(outcome) = ended(&mut model)? else {
        return Err("the resource catalog did not finish".into());
    };
    let summary = outcome.summary();
    let resources = summary
        .kind(Kind::ResourceTypes)
        .ok_or("no resource kind")?;
    assert_eq!(
        (resources.count, resources.skipped, resources.complete),
        (1, 0, true),
        "the selected resource is independently written"
    );
    let health = summary
        .export_health()
        .kind(Kind::ResourceTypes)
        .ok_or("no resource health")?;
    assert_eq!(
        health.source_records.selected, 1,
        "one physical resource source"
    );
    assert!(
        summary.export_health().is_complete(),
        "the source and projection completed"
    );
    let document: serde_json::Value = serde_json::from_slice(&fs::read(file)?)?;
    crate::catalog_resource_wire::assert_written_resource(&document, resources.count)?;
    Ok(())
}
