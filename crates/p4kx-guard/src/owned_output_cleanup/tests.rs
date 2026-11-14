// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{OwnedOutputCleanupError, cleanup_checked, remove_owned_output};
use crate::{AliasUncertainty, ArchiveWriteProtection, UnknownAlias, WriteProtectionError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixture() -> Result<(tempfile::TempDir, PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    fs::write(&source, b"synthetic source")?;
    let root = folder.path().join("owned-stage");
    fs::create_dir(&root)?;
    Ok((folder, source, root))
}

#[test]
fn ordinary_owned_output_is_cleaned_with_the_actual_cancelled_flag() -> TestResult {
    let (folder, source, root) = fixture()?;
    let child = root.join("sub");
    fs::create_dir(&child)?;
    fs::write(child.join("payload"), b"owned")?;
    fs::write(folder.path().join("sibling"), b"unrelated")?;
    let cancel = AtomicBool::new(false);
    let protection = ArchiveWriteProtection::new(&source, &cancel)?;
    cancel.store(true, Ordering::Relaxed);
    remove_owned_output(&root, &protection, &cancel)?;
    assert!(!root.exists());
    assert_eq!(fs::read(source)?, b"synthetic source");
    assert_eq!(fs::read(folder.path().join("sibling"))?, b"unrelated");
    Ok(())
}

#[test]
fn a_refused_child_stops_after_partial_cleanup_and_retains_its_typed_cause() -> TestResult {
    let (_folder, source, root) = fixture()?;
    for file in ["first", "second"] {
        fs::write(root.join(file), b"owned")?;
    }
    let first = RefCell::new(None::<PathBuf>);
    let result = cleanup_checked(&root, &|path| {
        if path.parent() == Some(root.as_path()) {
            let mut first = first.borrow_mut();
            if let Some(allowed) = first.as_ref() {
                if path != allowed {
                    return Err(WriteProtectionError::SourceMembership {
                        archive: source.clone(),
                        target: path.into(),
                        parent: root.clone(),
                    });
                }
            } else {
                *first = Some(path.into());
            }
        }
        Ok(())
    });
    let OwnedOutputCleanupError::Protection(WriteProtectionError::SourceMembership {
        target, ..
    }) = result.err().ok_or("child refusal missing")?
    else {
        return Err("child refusal lost its type".into());
    };
    let first = first.into_inner().ok_or("first owned file")?;
    assert!(!first.exists(), "one authorized leaf was removed");
    assert_eq!(fs::read(target)?, b"owned", "refused leaf is retained");
    assert!(root.is_dir());
    Ok(())
}

#[test]
fn changed_parent_identity_after_one_delete_prevents_remaining_removals() -> TestResult {
    let (_folder, _source, root) = fixture()?;
    for file in ["first", "second"] {
        fs::write(root.join(file), b"owned")?;
    }
    let first = RefCell::new(None::<PathBuf>);
    let result = cleanup_checked(&root, &|path| {
        if path == root && first.borrow().as_ref().is_some_and(|file| !file.exists()) {
            return Err(WriteProtectionError::TargetIdentity {
                target: path.into(),
                cause: UnknownAlias::new(AliasUncertainty::OwnerChanged),
            });
        }
        if path.parent() == Some(root.as_path()) && first.borrow().is_none() {
            *first.borrow_mut() = Some(path.into());
        }
        Ok(())
    });
    assert!(matches!(result, Err(OwnedOutputCleanupError::Protection(
        WriteProtectionError::TargetIdentity { cause, .. }
    )) if cause.reason() == AliasUncertainty::OwnerChanged));
    assert!(root.is_dir());
    assert_eq!(
        fs::read_dir(root)?.count(),
        1,
        "remaining leaf was not deleted"
    );
    Ok(())
}

#[test]
fn filesystem_failures_keep_the_original_io_cause() -> TestResult {
    let (_folder, source, _root) = fixture()?;
    let bad = source.with_file_name("bad\0node");
    let expected = fs::symlink_metadata(&bad)
        .err()
        .ok_or("invalid fixture path")?;
    let error = cleanup_checked(&bad, &|_| Ok(()))
        .err()
        .ok_or("invalid cleanup succeeded")?;
    let OwnedOutputCleanupError::Io { path, source } = error else {
        return Err("filesystem cause was replaced".into());
    };
    assert_eq!(path, bad);
    assert_eq!(source.kind(), expected.kind());
    assert_eq!(source.raw_os_error(), expected.raw_os_error());
    Ok(())
}

#[test]
fn the_root_and_parent_are_rechecked_after_the_owned_tree_is_removed() -> TestResult {
    let (_folder, _source, root) = fixture()?;
    fs::write(root.join("payload"), b"owned")?;
    let observations = RefCell::new(Vec::new());
    cleanup_checked(&root, &|path| {
        if path == root {
            observations.borrow_mut().push(root.exists());
        }
        Ok(())
    })?;
    let observations = observations.into_inner();
    assert_eq!(observations.first(), Some(&true));
    assert_eq!(observations.last(), Some(&false));
    assert!(root.parent().ok_or("owned parent")?.is_dir());
    Ok(())
}

#[cfg(unix)]
#[test]
fn directory_symlinks_are_removed_without_following_their_contents() -> TestResult {
    let (folder, source, root) = fixture()?;
    let external = folder.path().join("external");
    fs::create_dir(&external)?;
    fs::write(external.join("sentinel"), b"unrelated")?;
    std::os::unix::fs::symlink(&external, root.join("link"))?;
    let cancel = AtomicBool::new(false);
    let protection = ArchiveWriteProtection::new(&source, &cancel)?;
    remove_owned_output(&root, &protection, &cancel)?;
    assert!(!root.exists());
    assert_eq!(fs::read(external.join("sentinel"))?, b"unrelated");
    Ok(())
}

#[cfg(windows)]
#[test]
fn directory_junctions_are_removed_without_following_their_contents() -> TestResult {
    use std::os::windows::process::CommandExt as _;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let (folder, source, root) = fixture()?;
    let external = folder.path().join("external");
    fs::create_dir(&external)?;
    fs::write(external.join("sentinel"), b"unrelated")?;
    let script = folder.path().join("junction.ps1");
    fs::write(
        &script,
        "param([string]$Link,[string]$Target)\n$ErrorActionPreference='Stop'\nNew-Item -ItemType Junction -Path $Link -Target $Target | Out-Null\n",
    )?;
    let result = std::process::Command::new("pwsh")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-File"])
        .arg(script)
        .arg("-Link")
        .arg(root.join("link"))
        .arg("-Target")
        .arg(&external)
        .output()?;
    if !result.status.success() {
        return Err(std::io::Error::other("synthetic junction creation failed").into());
    }
    let cancel = AtomicBool::new(false);
    let protection = ArchiveWriteProtection::new(&source, &cancel)?;
    remove_owned_output(&root, &protection, &cancel)?;
    assert!(!root.exists());
    assert_eq!(fs::read(external.join("sentinel"))?, b"unrelated");
    Ok(())
}
