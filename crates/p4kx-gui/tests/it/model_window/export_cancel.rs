// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Cancelling an Export as GLB between reading the model and writing the file.

use crate::glb_exporting::{Window, start};
use crate::headless::{opened, ready};
use crate::viewing::{TestResult, select};
use crate::waiting::wait_for_model;
use p4kx_gui::{Language, ModelExport, ModelExportStage, ModelExportStatus};

#[test]
fn a_cancel_after_the_model_was_read_ends_cancelled_and_writes_no_file() -> TestResult {
    let (folder, mut harness): Window = opened(Language::BritishEnglish, ready())?;
    select(&mut harness, "cube.cgf")?;
    let hold = harness.state_mut().hold_model_export();
    let file = folder.path().join("cube.glb");
    assert!(start(&mut harness, &file)?, "started");
    wait_for_model(&mut harness, "the export to finish reading", |model| {
        matches!(
            model.model_export().map(ModelExport::status),
            Some(ModelExportStatus::Running(progress))
                if progress.stage == ModelExportStage::Writing
        )
    })?;

    harness.state().cancel_model_export();
    wait_for_model(&mut harness, "the export to end", |model| {
        model
            .model_export()
            .is_some_and(|export| !export.is_running())
    })?;
    drop(hold);

    let status = harness
        .state()
        .model_export()
        .map(ModelExport::status)
        .ok_or("no export")?;
    assert!(
        matches!(status, ModelExportStatus::Cancelled),
        "cancelled, not {status:?}"
    );
    assert!(!file.exists(), "no file");
    assert!(
        !file.with_extension("glb.p4kx-origin.jsonl").exists(),
        "cancelled export publishes no origin receipt"
    );
    Ok(())
}
