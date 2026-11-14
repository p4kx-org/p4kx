// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The P4KX catalog of the window over a real Archive; runs with `--ignored` when
//! `P4KX_DATA_P4K` names one.

use std::error::Error;
use std::fs;
use std::time::{Duration, Instant};

use p4kx_export::{Kind, SizeWarning};
use p4kx_gui::{CatalogExport, CatalogExportStatus, Language, Model};
use p4kx_testkit::real_archive_path;

type TestResult = Result<(), Box<dyn Error>>;

fn wait_until(model: &mut Model, done: impl Fn(&Model) -> bool) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(600);
    while !done(model) {
        if Instant::now() > deadline {
            return Err("the window did not settle in time".into());
        }
        std::thread::sleep(Duration::from_millis(10));
        model.poll();
        model.poll_catalog();
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_catalog_of_a_real_archive_is_written_with_its_summary() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("p4kx-catalog.json");
    let mut model = Model::new(Language::BritishEnglish);
    model.set_archive_key(Some(p4kx_testkit::real_archive_key()?));
    model.open(real_archive_path()?, || {});
    wait_until(&mut model, |model| model.browser().is_some())?;

    if !model.start_catalog(file.clone(), || {}) {
        return Err("the catalog did not start".into());
    }
    wait_until(&mut model, |model| {
        model.catalog().is_some_and(|run| !run.is_running())
    })?;
    let Some(CatalogExportStatus::Finished(outcome)) = model.catalog().map(CatalogExport::status)
    else {
        return Err("no summary".into());
    };
    let summary = outcome.summary();

    let makers = summary
        .kind(Kind::Manufacturers)
        .ok_or("no manufacturers")?;
    assert!(makers.count > 50, "many manufacturers: {}", makers.count);
    assert_ne!(
        summary.decompressed_warning(),
        Some(SizeWarning::OverLimit),
        "within the decompressed limit"
    );
    assert_eq!(
        fs::metadata(&file)?.len(),
        summary.size(),
        "the file holds the catalog"
    );
    assert!(
        model.record_export_keeps_dataforge(),
        "DataForge is kept for the next export"
    );
    Ok(())
}
