// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Arc;

use p4kx_export::{CancelToken, ConflictPolicy, RecordForm};

use super::{Job, Request, Selection};
use crate::TargetKind;
use crate::forge_cache::tests::{opened, write_archive};
use crate::record_export::progress::Shared;

type TestResult = Result<(), Box<dyn Error>>;

fn run(tags: bool) -> Result<(bool, usize, usize), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    write_archive(&path, true)?;
    let model = opened(&path)?;
    let index = Arc::clone(model.browser().ok_or("no Archive")?.shared_index());
    let request = Request {
        selection: Selection::Records(Arc::from([0_u32])),
        form: RecordForm::Json,
        tags,
        kind: TargetKind::ExistingFolder,
        folder: dir.path().join("out"),
        policy: ConflictPolicy::Skip,
    };
    let cancel = CancelToken::new();
    cancel.cancel();
    let shared = Shared::default();
    let job = Job {
        index: &index,
        request: &request,
        cancel: &cancel,
        shared: &shared,
        #[cfg(feature = "test-hooks")]
        pause: crate::hold::Pause::default(),
    };
    let summary = job.run()?;
    Ok((
        summary.was_cancelled(),
        summary.written(),
        index.forge().tag_builds(),
    ))
}

#[test]
fn a_cancel_before_the_tags_are_built_ends_the_export_as_cancelled_and_builds_nothing() -> TestResult
{
    let (cancelled, written, builds) = run(true)?;

    assert!(cancelled, "cancelled");
    assert_eq!(written, 0, "nothing written");
    assert_eq!(builds, 0, "the stopped build stored no tag tree");
    Ok(())
}

#[test]
fn a_cancel_without_the_tags_option_ends_the_export_as_cancelled_too() -> TestResult {
    let (cancelled, written, builds) = run(false)?;

    assert!(cancelled, "cancelled");
    assert_eq!(written, 0, "nothing written");
    assert_eq!(builds, 0, "no tag tree asked for");
    Ok(())
}
