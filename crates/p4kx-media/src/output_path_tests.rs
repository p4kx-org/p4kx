// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{
    Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken, WriteProtectionError,
};
use p4kx_testkit::ArchiveBuilder;

use super::ensure_directory;
use crate::MediaError;
use crate::checkpoint::Checkpoint;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn with_source(run: impl FnOnce(&Checkpoint<'_>, &Path, &Path) -> TestResult) -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new().write_to(&path)?;
    let archive = Archive::open(&path)?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(&path, cancel.flag())?;
    let watch = ArchiveWatch::new(&archive);
    run(
        &Checkpoint {
            cancel: &cancel,
            watch: &watch,
            protection: &protection,
        },
        folder.path(),
        &path,
    )
}

#[test]
fn existing_prefixes_are_preserved_while_missing_descendants_are_created() -> TestResult {
    with_source(|checkpoint, folder, _| {
        let parent = folder.join("exports");
        fs::create_dir(&parent)?;
        fs::write(parent.join("foreign"), b"foreign bytes")?;
        let target = parent.join("new").join("nested");
        ensure_directory(&target, checkpoint)?;
        assert!(target.is_dir());
        assert_eq!(fs::read(parent.join("foreign"))?, b"foreign bytes");
        Ok(())
    })
}

#[test]
fn the_real_directory_target_is_still_refused_when_it_contains_the_source() -> TestResult {
    with_source(|checkpoint, folder, source| {
        let original = fs::read(source)?;
        assert!(matches!(
            ensure_directory(folder, checkpoint),
            Err(MediaError::Protected(
                WriteProtectionError::SourceOverwrite { .. }
            ))
        ));
        assert_eq!(fs::read(source)?, original);
        assert_eq!(fs::read_dir(folder)?.count(), 1);
        Ok(())
    })
}

#[test]
fn parent_traversal_is_refused_before_creating_any_prefix_directory() -> TestResult {
    with_source(|checkpoint, folder, _| {
        let target = folder.join("uncreated").join("..").join("escape");
        assert!(matches!(
            ensure_directory(&target, checkpoint),
            Err(MediaError::Destination(_))
        ));
        assert!(!folder.join("uncreated").exists());
        assert!(!folder.join("escape").exists());
        Ok(())
    })
}
