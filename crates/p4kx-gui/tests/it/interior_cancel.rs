// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Stopping the export of an Interior container.

use p4kx_gui::{AssemblyExport, AssemblyExportStage, AssemblyExportStatus};
use p4kx_testkit::INTERIOR_ROOT;

use crate::entity_picking::TestResult;
use crate::interior_export::{container_window, finished, select_socpak};
use crate::waiting::wait_for_model;

#[test]
fn the_interior_export_can_be_cancelled_and_leaves_no_file() -> TestResult {
    let (dir, mut harness) = container_window()?;
    select_socpak(&mut harness, INTERIOR_ROOT)?;
    harness.state_mut().begin_interior_export();
    let file = dir.path().join("cancelled.glb");
    harness
        .state_mut()
        .assembly_export_setup_mut()
        .ok_or("no setup")?
        .set_file(file.clone());
    let hold = harness.state_mut().hold_interior_export();
    assert!(harness.state_mut().submit_assembly_export(|| {}));
    let running = harness.state().assembly_export().ok_or("no export")?;
    assert!(running.is_container(), "an Interior export");
    wait_for_model(&mut harness, "the export to reach the Interior", |model| {
        matches!(
            model.assembly_export().map(AssemblyExport::status),
            Some(AssemblyExportStatus::Running(progress))
                if progress.stage == AssemblyExportStage::Interior
        )
    })?;

    harness.state().cancel_assembly_export();
    finished(&mut harness)?;
    drop(hold);

    let status = harness
        .state()
        .assembly_export()
        .map(AssemblyExport::status)
        .ok_or("no export")?;
    assert!(
        matches!(status, AssemblyExportStatus::Cancelled),
        "cancelled, not {status:?}"
    );
    assert!(!file.exists(), "no file");
    assert!(
        !file.with_extension("glb.p4kx-origin.jsonl").exists(),
        "no orphan receipt"
    );
    Ok(())
}
