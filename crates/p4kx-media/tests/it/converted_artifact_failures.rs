// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;

use p4kx_extraction::{CancelToken, ConflictPolicy};
use p4kx_media::{ExportOptions, ExportOutcome, MediaError};

use super::converted_artifact_support::{self as encoded, Mode};
use super::support::{self, TestResult};

#[test]
fn actual_encoder_io_failure_preserves_the_previous_index_and_its_validated_skip() -> TestResult {
    assert_failure(Mode::Io)
}

#[test]
fn cancellation_inside_encoding_preserves_the_previous_index_and_its_validated_skip() -> TestResult
{
    assert_failure(Mode::Cancelled)
}

#[test]
fn an_ignored_actual_output_quota_error_preserves_the_previous_index_and_validated_skip()
-> TestResult {
    assert_failure(Mode::IgnoredQuota)
}

fn assert_failure(mode: Mode) -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = encoded::archive(folder.path())?;
    let output = folder.path().join("media");
    let original = encoded::registry(Mode::Success, CancelToken::new())?;
    support::published(p4kx_media::export_with(
        &archive,
        &output,
        &ExportOptions::default(),
        &original,
    )?)?;
    let previous = fs::read(output.join("media-index.json"))?;
    let cancel = CancelToken::new();
    let failing = encoded::registry(mode, cancel.clone())?;
    let mut options = ExportOptions {
        conflict: ConflictPolicy::Overwrite,
        cancel,
        ..ExportOptions::default()
    };
    if matches!(mode, Mode::IgnoredQuota) {
        options.limits.output_bytes = 16 << 10;
    }
    let failure = p4kx_media::export_with(&archive, &output, &options, &failing)
        .err()
        .ok_or("failed encoding published")?;
    assert!(
        encoded::contains(&failure, |cause| match mode {
            Mode::Io =>
                matches!(cause, MediaError::Io { source, .. } if source.kind() == io::ErrorKind::NotFound && source.raw_os_error().is_some()),
            Mode::Cancelled => matches!(cause, MediaError::Cancelled),
            Mode::IgnoredQuota => matches!(
                cause,
                MediaError::Limit {
                    budget: "output_bytes",
                    ..
                }
            ),
            Mode::Success | Mode::Repeat | Mode::Empty => false,
        }),
        "original typed encoder failure stays reachable: {failure:?}"
    );
    assert_eq!(
        fs::read(output.join("media-index.json"))?,
        previous,
        "failed attempt never replaces the complete index"
    );
    assert!(
        matches!(
            p4kx_media::export_with(&archive, &output, &ExportOptions::default(), &original)?,
            ExportOutcome::Skipped { .. }
        ),
        "previous generation remains actually valid"
    );
    assert_eq!(
        fs::read(output.join("media-index.json"))?,
        previous,
        "validated skip preserves previous index bytes"
    );
    Ok(())
}
