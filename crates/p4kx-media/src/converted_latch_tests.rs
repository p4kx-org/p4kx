// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use p4kx_extraction::Exclusion;

use crate::required_input_fault_support::{self as support, Inventory, TestResult};
use crate::{ArtifactExtension, MediaError};

#[test]
fn a_late_source_change_after_a_sticky_quota_failure_keeps_both_causes() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let remaining = writer.remaining_output_bytes();
            writer.limits.output_bytes -= remaining;
            let archive = context.directory.index.archive.path().to_path_buf();
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |output| {
                    assert!(output.write(b"data").is_err(), "actual sticky quota error");
                    change(&archive)?;
                    Ok(())
                })
                .err()
                .ok_or("late checkpoint error was ignored")?;
            let MediaError::OutputCheckFailed {
                operation,
                checkpoint,
            } = failure
            else {
                return Err("independent final checkpoint failure was not retained".into());
            };
            assert!(matches!(*checkpoint, MediaError::Changed(_)));
            let MediaError::Output {
                cause,
                callback: None,
            } = *operation
            else {
                return Err("original sticky quota failure".into());
            };
            assert!(matches!(
                cause.as_ref(),
                MediaError::Limit {
                    budget: "output_bytes",
                    ..
                }
            ));
            assert_eq!(
                fs::read_dir(writer.root.join("chunks"))?
                    .collect::<Result<Vec<_>, _>>()?
                    .len(),
                0
            );
            Ok(())
        },
    )
}

#[test]
fn a_latched_source_change_does_not_become_a_second_independent_failure() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, _| {
            let archive = context.directory.index.archive.path().to_path_buf();
            let mut held = None;
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |output| {
                    change(&archive)?;
                    held = Some(output.write(b"data").err().ok_or_else(|| {
                        MediaError::Contract("changed source was ignored".into())
                    })?);
                    Ok(())
                })
                .err()
                .ok_or("source change published")?;
            let MediaError::Output {
                cause,
                callback: None,
            } = failure
            else {
                return Err("repeated source latch changed the failure category".into());
            };
            assert!(matches!(cause.as_ref(), MediaError::Changed(_)));
            assert!(held.is_some());
            Ok(())
        },
    )
}

#[test]
fn a_latched_cancellation_does_not_become_a_second_independent_failure() {
    let reached = Cell::new(false);
    let result = support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, writer, checkpoint, _| {
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |output| {
                    checkpoint.cancel.cancel();
                    assert!(output.write(b"data").is_err(), "actual sink cancellation");
                    Ok(())
                })
                .err()
                .ok_or("cancelled encoding published")?;
            assert_one_stop(&failure);
            reached.set(true);
            Ok(())
        },
    );
    assert!(
        reached.get(),
        "actual cancellation assertions ran before cleanup"
    );
    if let Err(error) = result {
        assert!(
            matches!(
                error.downcast_ref::<MediaError>(),
                Some(MediaError::Cleanup(_) | MediaError::CleanupFailed { .. })
            ),
            "only typed protected cleanup refusal may remain: {error}"
        );
    }
}

fn assert_one_stop(failure: &MediaError) {
    use std::error::Error;
    let mut current: Option<&(dyn Error + 'static)> = Some(failure);
    let mut cancelled = false;
    while let Some(error) = current {
        if let Some(cause) = error.downcast_ref::<MediaError>() {
            assert!(
                !matches!(cause, MediaError::OutputCheckFailed { .. }),
                "same bound cancellation was spuriously counted twice"
            );
            cancelled |= matches!(cause, MediaError::Cancelled);
        }
        current = error.source();
    }
    assert!(cancelled, "original cancellation remains typed");
}

fn change(path: &Path) -> Result<(), MediaError> {
    OpenOptions::new()
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(b"actual synthetic source change"))
        .map_err(|source| MediaError::Io {
            path: path.into(),
            source,
        })
}

#[test]
fn an_actual_callback_checkpoint_change_does_not_become_a_second_failure() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, checkpoint, _| {
            let archive = context.directory.index.archive.path().to_path_buf();
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |_output| {
                    change(&archive)?;
                    checkpoint.check()
                })
                .err()
                .ok_or("callback source change published")?;
            assert!(
                matches!(failure, MediaError::Changed(_)),
                "same path and the actual latched Watch prove a repeated source failure"
            );
            Ok(())
        },
    )
}

#[test]
fn actual_callback_io_and_a_later_source_change_keep_both_terminal_causes() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, folder| {
            let original = fs::File::open(folder.join("missing-callback-input"))
                .err()
                .ok_or("missing callback input opened")?;
            let expected = (original.kind(), original.raw_os_error());
            let archive = context.directory.index.archive.path().to_path_buf();
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |_output| {
                    change(&archive)?;
                    Err(MediaError::Io {
                        path: "callback input".into(),
                        source: original,
                    })
                })
                .err()
                .ok_or("two callback failures published")?;
            let MediaError::OutputCheckFailed {
                operation,
                checkpoint,
            } = failure
            else {
                return Err("both callback and final checkpoint causes retained".into());
            };
            let MediaError::Io { source, .. } = *operation else {
                return Err("original callback I/O type".into());
            };
            assert_eq!((source.kind(), source.raw_os_error()), expected);
            assert!(matches!(*checkpoint, MediaError::Changed(_)));
            Ok(())
        },
    )
}

#[test]
fn a_callback_change_for_a_different_source_does_not_hide_the_final_owning_change() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, writer, _, folder| {
            let foreign = folder.join("foreign-source.bin");
            fs::write(&foreign, b"original")?;
            let foreign_watch = p4kx_extraction::ArchiveWatch::of_file(&foreign)?;
            let own = context.directory.index.archive.path().to_path_buf();
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |_output| {
                    change(&foreign)?;
                    change(&own)?;
                    foreign_watch.check().map_err(MediaError::from)
                })
                .err()
                .ok_or("distinct changes published")?;
            let MediaError::OutputCheckFailed {
                operation,
                checkpoint,
            } = failure
            else {
                return Err("different physical sources were deduplicated".into());
            };
            assert!(
                matches!(*operation, MediaError::Changed(ref source) if source.path() == foreign.as_path())
            );
            assert!(
                matches!(*checkpoint, MediaError::Changed(ref source) if source.path() == own.as_path())
            );
            Ok(())
        },
    )
}
