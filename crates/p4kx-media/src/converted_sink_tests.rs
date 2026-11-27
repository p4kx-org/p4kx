// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io;

use p4kx_extraction::Exclusion;

use crate::required_input_fault_support::{self as support, Inventory, TestResult};
use crate::{ArtifactExtension, MediaError};

#[test]
fn unknown_length_bytes_are_admitted_before_forwarding_and_ignored_errors_stay_typed() -> TestResult
{
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, writer, _, _| {
            let admitted = writer.limits.output_bytes - writer.remaining_output_bytes();
            writer.limits.output_bytes = admitted + 3;
            let mut held = None;
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |output| {
                    assert_eq!(
                        output.write(b"first").map_err(io_error)?,
                        3,
                        "one partial bounded write uses the remaining quota"
                    );
                    held = Some(
                        output
                            .write(b"last")
                            .err()
                            .ok_or_else(|| MediaError::Contract("quota was ignored".into()))?,
                    );
                    Ok(())
                })
                .err()
                .ok_or("ignored write failure published")?;
            assert_eq!(writer.remaining_output_bytes(), 0);
            assert_quota(&failure)?;
            assert!(
                held.is_some(),
                "encoder can retain the same original I/O error after returning"
            );
            assert_no_temporaries(&writer.root)?;
            Ok(())
        },
    )
}

#[test]
fn an_independent_callback_error_does_not_replace_a_sticky_output_error() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, writer, _, folder| {
            let admitted = writer.limits.output_bytes - writer.remaining_output_bytes();
            writer.limits.output_bytes = admitted;
            let original = File::open(folder.join("missing-encoder-input"))
                .err()
                .ok_or("missing input opened")?;
            let expected = (original.kind(), original.raw_os_error());
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |output| {
                    assert!(output.write(b"data").is_err(), "actual sink quota failure");
                    Err(MediaError::Io {
                        path: "encoder input".into(),
                        source: original,
                    })
                })
                .err()
                .ok_or("two failures published")?;
            let MediaError::Output {
                cause,
                callback: Some(callback),
            } = failure
            else {
                return Err("both output and callback causes".into());
            };
            assert!(matches!(
                cause.as_ref(),
                MediaError::Limit {
                    budget: "output_bytes",
                    ..
                }
            ));
            let MediaError::Io { source, .. } = *callback else {
                return Err("original callback I/O type".into());
            };
            assert_eq!((source.kind(), source.raw_os_error()), expected);
            assert_no_temporaries(&writer.root)?;
            Ok(())
        },
    )
}

#[test]
fn a_callback_io_error_after_real_bytes_keeps_its_original_os_error() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, writer, _, folder| {
            let original = File::open(folder.join("missing-callback-input"))
                .err()
                .ok_or("missing input opened")?;
            let expected = (original.kind(), original.raw_os_error());
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |output| {
                    output.write_all(b"partial").map_err(io_error)?;
                    Err(MediaError::Io {
                        path: "callback input".into(),
                        source: original,
                    })
                })
                .err()
                .ok_or("failed encoder published")?;
            let MediaError::Io { source, .. } = failure else {
                return Err("callback I/O type".into());
            };
            assert_eq!((source.kind(), source.raw_os_error()), expected);
            assert_no_temporaries(&writer.root)?;
            Ok(())
        },
    )
}

#[test]
fn actual_io_and_post_operation_archive_change_keep_both_causes() -> TestResult {
    use std::error::Error as _;
    use std::io::Write as _;
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, _writer, checkpoint, folder| {
            let original = File::open(folder.join("not-present"))
                .err()
                .ok_or("missing input opened")?;
            let expected = (original.kind(), original.raw_os_error());
            std::fs::OpenOptions::new()
                .append(true)
                .open(context.directory.index.archive.path())?
                .write_all(b"actual source change")?;
            let result: Result<(), MediaError> = Err(MediaError::Io {
                path: "encoded output".into(),
                source: original,
            });
            let failure = crate::output_operation::checked(result, checkpoint.check())
                .err()
                .ok_or("combined output failure")?;
            assert!(
                failure
                    .source()
                    .and_then(|cause| cause.downcast_ref::<MediaError>())
                    .is_some_and(|cause| matches!(cause, MediaError::Io { .. }))
            );
            let MediaError::OutputCheckFailed {
                operation,
                checkpoint,
            } = failure
            else {
                return Err("original output/checkpoint causes".into());
            };
            let MediaError::Io { source, .. } = *operation else {
                return Err("original output I/O type".into());
            };
            assert_eq!((source.kind(), source.raw_os_error()), expected);
            assert!(matches!(*checkpoint, MediaError::Changed(_)));
            Ok(())
        },
    )
}

fn assert_no_temporaries(root: &std::path::Path) -> TestResult {
    assert_eq!(
        std::fs::read_dir(root.join("chunks"))?
            .collect::<Result<Vec<_>, _>>()?
            .len(),
        0,
        "failed write left no placed or temporary artifact"
    );
    Ok(())
}

fn assert_quota(failure: &MediaError) -> TestResult {
    use std::error::Error as _;
    let MediaError::Output {
        cause,
        callback: None,
    } = failure
    else {
        return Err("sticky output type".into());
    };
    assert!(matches!(
        cause.as_ref(),
        MediaError::Limit {
            budget: "output_bytes",
            ..
        }
    ));
    assert!(
        matches!(
            failure
                .source()
                .and_then(|cause| cause.downcast_ref::<MediaError>()),
            Some(MediaError::Limit {
                budget: "output_bytes",
                ..
            })
        ),
        "shared source reaches the original MediaError rather than an Arc wrapper"
    );
    Ok(())
}

fn io_error(source: io::Error) -> MediaError {
    MediaError::Io {
        path: "synthetic encoder".into(),
        source,
    }
}
