// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Headless Query selection distinguishes read failures, real emptiness and complete exports.

use std::fs;

use p4kx_gui::{QueryExportFormat, QueryExportStatus, QueryStatus};
use p4kx_testkit::query_selection_sample;

use crate::querying::{self, TestResult, model};
use crate::waiting;

#[test]
fn a_late_witness_is_shown_and_exported_and_unknowns_remain_empty() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut model = model::opened(&querying::archive(
        dir.path(),
        &query_selection_sample::database(),
    )?)?;
    model::run(&mut model, "from Station select id where tags[*] = 4096")?;
    assert_eq!(
        model::finished(&model)?.len(),
        1,
        "the late witness is selected"
    );
    let out = dir.path().join("result.json");
    assert!(
        model
            .query_mut()
            .export(out.clone(), QueryExportFormat::Json, || {}),
        "the complete result can be exported"
    );
    waiting::wait_polling(
        &mut model,
        "the Query export",
        |model| {
            model.query_mut().poll();
        },
        |model| !model.query().is_exporting(),
    )?;
    assert!(
        matches!(
            model.query().export_status(),
            Some(QueryExportStatus::Exported {
                records: 1,
                failed: 0,
                ..
            })
        ),
        "one complete row was exported"
    );
    assert_eq!(
        fs::read_to_string(out)?,
        "[{\"record path\":\"synthetic/station\",\"id\":7}]\n",
        "the exported row agrees with selection"
    );
    model::run(&mut model, "from Station where owner.ready = true")?;
    assert!(
        model::finished(&model)?.is_empty(),
        "a normal unknown has a complete empty result"
    );
    Ok(())
}

#[test]
fn a_demanded_read_failure_has_no_successful_result_or_export() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut model = model::opened(&querying::archive(
        dir.path(),
        &query_selection_sample::database(),
    )?)?;
    let out = dir.path().join("result.json");
    fs::write(&out, b"complete earlier export")?;
    model::run(&mut model, "from Station where ready = true")?;
    let QueryStatus::Failed(Some(error)) = model.query().status() else {
        return Err(format!("not failed: {:?}", model.query().status()).into());
    };
    assert!(
        error.contains("synthetic/station")
            && error.contains("comparison 1")
            && error.contains("bool byte 2"),
        "{error}"
    );
    assert!(
        model.query().result().is_none(),
        "a failure has no successful result"
    );
    assert!(
        !model
            .query_mut()
            .export(out.clone(), QueryExportFormat::Json, || {}),
        "a failed selection cannot be exported"
    );
    assert_eq!(
        fs::read(out)?,
        b"complete earlier export",
        "the prior file is unchanged"
    );
    Ok(())
}
