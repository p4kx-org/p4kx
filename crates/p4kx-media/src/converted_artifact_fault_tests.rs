// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use p4kx_extraction::{Checksum, Exclusion};
use sha2::{Digest as _, Sha256};

use crate::required_input_fault_support::{self as support, Inventory, TestResult};
use crate::{ArtifactExtension, MediaError};

#[test]
fn a_real_content_address_collision_does_not_return_an_artifact_or_replace_bytes() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, writer, _, _| {
            let digest = Checksum::Sha256(Sha256::digest(b"data").into()).hex();
            let target = writer.root.join(format!("chunks/{digest}.bin"));
            fs::write(&target, b"evil")?;
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |output| {
                    output.write_all(b"data").map_err(io_error)
                })
                .err()
                .ok_or("collision returned a valid artifact")?;
            assert!(
                matches!(failure, MediaError::Contract(message) if message == "converted artifact content-address collision")
            );
            assert_eq!(
                fs::read(&target)?,
                b"evil",
                "unverified existing bytes are never replaced"
            );
            assert_eq!(
                fs::read_dir(writer.root.join("chunks"))?.count(),
                1,
                "failed collision cleaned its own temporary"
            );
            Ok(())
        },
    )
}

#[test]
fn callback_success_cannot_hide_a_temporary_identity_and_cleanup_failure() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, writer, _, folder| {
            let directory = writer.root.join("chunks");
            let parked = folder.join("parked-encoded-file");
            let mut original = None;
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |output| {
                    output.write_all(b"data").map_err(io_error)?;
                    original = Some(swap_temporary(&directory, &parked)?);
                    Ok(())
                })
                .err()
                .ok_or("changed temporary became an artifact")?;
            let MediaError::CleanupFailed { primary, cleanup } = failure else {
                return Err("both output and cleanup failures retained".into());
            };
            assert!(
                matches!(*primary, MediaError::Protected(_)),
                "sync refuses the replaced physical file"
            );
            assert!(
                matches!(*cleanup, MediaError::Cleanup(_)),
                "cleanup keeps its typed failure"
            );
            let path = original.ok_or("actual temporary was not replaced")?;
            assert_eq!(
                fs::read(&path)?,
                b"replacement",
                "cleanup did not unlink the replacement"
            );
            assert_eq!(
                fs::read(&parked)?,
                b"data",
                "actual written bytes remain attributable"
            );
            fs::remove_file(&path)?;
            fs::rename(&parked, &path)?;
            Ok(())
        },
    )
}

#[test]
fn artifact_creation_outside_a_callback_is_refused_before_the_encoder_runs() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, writer, _, _| {
            writer.end_directory()?;
            let mut called = false;
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |_output| {
                    called = true;
                    Ok(())
                })
                .err()
                .ok_or("inactive output succeeded")?;
            assert!(!called);
            assert!(matches!(failure, MediaError::Contract(_)));
            assert_eq!(fs::read_dir(writer.root.join("chunks"))?.count(), 0);
            Ok(())
        },
    )
}

fn io_error(source: io::Error) -> MediaError {
    MediaError::Io {
        path: "synthetic encoder".into(),
        source,
    }
}

fn swap_temporary(directory: &Path, parked: &Path) -> Result<PathBuf, MediaError> {
    let entries = fs::read_dir(directory)
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .map_err(|source| MediaError::Io {
            path: directory.into(),
            source,
        })?;
    if entries.len() != 1 {
        return Err(MediaError::Contract("one actual temporary encoding".into()));
    }
    let path = entries
        .first()
        .ok_or_else(|| MediaError::Contract("actual temporary disappeared".into()))?
        .path();
    fs::rename(&path, parked).map_err(|source| MediaError::Io {
        path: path.clone(),
        source,
    })?;
    fs::write(&path, b"replacement").map_err(|source| MediaError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}
