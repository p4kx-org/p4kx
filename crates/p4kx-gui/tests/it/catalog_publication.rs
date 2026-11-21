// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Read;
use std::path::Path;

use p4kx_export::{CatalogPublicationMode, Filter, Kind};
use p4kx_gui::{CatalogExportOptions, CatalogExportStatus};

use crate::cataloging::{self, TestResult, ended, files_in, opened};

fn read_catalog(path: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    if path.extension().is_some_and(|extension| extension == "zst") {
        let mut decoded = Vec::new();
        zstd::Decoder::new(bytes.as_slice())?.read_to_end(&mut decoded)?;
        Ok(serde_json::from_slice(&decoded)?)
    } else {
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[test]
fn normal_source_failures_preserve_fresh_and_existing_destinations() -> TestResult {
    for extension in ["json", "json.zst"] {
        for existing in [false, true] {
            let (_dir, mut model, out) = opened(&cataloging::unreadable())?;
            let file = out.join(format!("catalog.{extension}"));
            if existing {
                fs::write(&file, "verified previous snapshot")?;
            }
            assert!(model.start_catalog(file.clone(), || {}), "the job starts");
            let CatalogExportStatus::Incomplete(health) = ended(&mut model)? else {
                return Err("an incomplete normal job must refuse publication".into());
            };
            assert!(!health.is_complete(), "the refusal carries real health");
            if existing {
                assert_eq!(
                    fs::read(&file)?.as_slice(),
                    b"verified previous snapshot",
                    "existing bytes survive"
                );
                assert_eq!(files_in(&out)?.len(), 1, "no temporary artifact remains");
            } else {
                assert!(
                    files_in(&out)?.is_empty(),
                    "no destination or temporary artifact"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn diagnostic_permission_is_explicit_per_job_and_is_carried_by_the_artifact() -> TestResult {
    for extension in ["json", "json.zst"] {
        for existing in [false, true] {
            let (_dir, mut model, out) = opened(&cataloging::unreadable())?;
            let file = out.join(format!("diagnostic.{extension}"));
            if existing {
                fs::write(&file, "old")?;
            }
            let options = CatalogExportOptions {
                publication_mode: CatalogPublicationMode::DiagnosticPartial,
                ..CatalogExportOptions::default()
            };
            assert!(
                model.start_catalog_with_options(file.clone(), options, || {}),
                "explicit diagnostic job starts"
            );
            let CatalogExportStatus::Finished(outcome) = ended(&mut model)? else {
                return Err("the explicit diagnostic artifact must be written".into());
            };
            assert!(
                !outcome.summary().is_complete(),
                "written does not mean complete"
            );
            let artifact = read_catalog(&file)?;
            assert_eq!(
                artifact["exportHealth"]["status"], "incomplete",
                "file-only consumer sees failure"
            );
            assert_eq!(
                artifact["exportHealth"]["publicationMode"], "diagnosticPartial",
                "explicit mode survives serialization"
            );
            assert_eq!(
                artifact["exportHealth"],
                serde_json::to_value(outcome.summary().export_health())?,
                "memory and file agree"
            );
            model.close_catalog();
            let normal = out.join("next-normal.json");
            assert!(
                model.start_catalog(normal.clone(), || {}),
                "a later default job starts"
            );
            assert!(
                matches!(ended(&mut model)?, CatalogExportStatus::Incomplete(_)),
                "diagnostic permission is not retained"
            );
            assert!(
                !normal.exists(),
                "default normal publication remains protected"
            );
        }
    }
    Ok(())
}

#[test]
fn a_healthy_filter_publishes_with_separate_selection_completeness() -> TestResult {
    let (_dir, mut model, out) = opened(&cataloging::unreadable())?;
    let file = out.join("filtered.json");
    let options = CatalogExportOptions {
        filter: Some(Filter::parse("scitemmanufacturer")?),
        ..CatalogExportOptions::default()
    };
    assert!(
        model.start_catalog_with_options(file.clone(), options, || {}),
        "filtered job starts"
    );
    let CatalogExportStatus::Finished(outcome) = ended(&mut model)? else {
        return Err("the selected readable manufacturer is publishable".into());
    };
    assert!(
        outcome.summary().is_complete(),
        "selected reads/projections are healthy"
    );
    assert!(
        !outcome
            .summary()
            .kind(Kind::Manufacturers)
            .ok_or("manufacturer kind")?
            .complete,
        "the Filter remains partial selection"
    );
    assert_eq!(
        read_catalog(&file)?["exportHealth"]["publicationMode"],
        "normal",
        "no diagnostic fallback"
    );
    Ok(())
}
