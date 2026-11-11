// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{
    Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken, WriteProtectionError,
};
use p4kx_testkit::ArchiveBuilder;

use super::ensure_directory;
#[cfg(unix)]
use super::{directory, file};
use crate::MediaError;
use crate::checkpoint::Checkpoint;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn with_source(run: impl FnOnce(&Checkpoint<'_>, &Path, &Path) -> TestResult) -> TestResult {
    let folder = tempfile::tempdir()?;
    with_source_in(folder.path(), run)
}

fn with_source_in(
    folder: &Path,
    run: impl FnOnce(&Checkpoint<'_>, &Path, &Path) -> TestResult,
) -> TestResult {
    let path = folder.join("source.p4k");
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
        folder,
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

#[cfg(target_os = "macos")]
#[test]
fn macos_temporary_aliases_allow_plain_output_and_retain_source_protection() -> TestResult {
    for base in ["/var/tmp", "/tmp"] {
        let folder = tempfile::tempdir_in(base)?;
        with_source_in(folder.path(), |checkpoint, folder, source| {
            let original = fs::read(source)?;
            let target = folder.join("exports").join("nested");
            ensure_directory(&target, checkpoint)?;
            assert!(directory(&target)?);
            let artifact = target.join("artifact");
            fs::write(&artifact, b"exported bytes")?;
            assert!(file(&artifact)?);
            assert!(matches!(
                ensure_directory(folder, checkpoint),
                Err(MediaError::Protected(
                    WriteProtectionError::SourceOverwrite { .. }
                ))
            ));
            assert_eq!(fs::read(source)?, original);
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn an_output_link_to_the_protected_source_directory_is_refused() -> TestResult {
    with_source(|checkpoint, folder, source| {
        let original = fs::read(source)?;
        let alias = folder.join("redirect");
        std::os::unix::fs::symlink(folder, &alias)?;
        assert!(matches!(
            ensure_directory(&alias.join("new"), checkpoint),
            Err(MediaError::Destination(_))
        ));
        assert!(matches!(directory(&alias), Err(MediaError::Destination(_))));
        assert!(matches!(
            file(&alias.join("source.p4k")),
            Err(MediaError::Destination(_))
        ));
        assert!(!folder.join("new").exists());
        assert_eq!(fs::read(source)?, original);
        Ok(())
    })
}
