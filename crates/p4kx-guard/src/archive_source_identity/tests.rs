// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};

use same_file::Handle;

use super::ArchiveSourceIdentity;
use crate::{AliasUncertainty, WriteProtectionError};

#[test]
fn one_target_spelling_uses_one_identity_snapshot() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let archive = root.path().join("source.p4k");
    fs::write(&archive, b"source")?;
    let identity = ArchiveSourceIdentity::new(&archive, &archive)?;
    let lock = root.path().join(".p4kx-origin.lock");
    File::create_new(&lock)?;

    identity.check_content_with(&lock, &lock, false, || {
        fs::rename(&lock, root.path().join("retired.lock"))?;
        File::create_new(&lock)?;
        Handle::from_path(&lock)
    })?;
    assert!(!root.path().join("retired.lock").exists());
    Ok(())
}

#[test]
fn distinct_target_spellings_require_matching_file_identities()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let archive = root.path().join("source.p4k");
    fs::write(&archive, b"source")?;
    let identity = ArchiveSourceIdentity::new(&archive, &archive)?;
    let visible = root.path().join("visible.txt");
    let host = root.path().join("host.txt");
    fs::write(&visible, b"visible")?;
    fs::write(&host, b"host")?;

    let refusal = identity
        .check(&visible, &host)
        .expect_err("different file identities at the target cannot authorize a write");
    assert!(matches!(
        refusal,
        WriteProtectionError::TargetIdentity { cause, .. }
            if cause.reason() == AliasUncertainty::HostFileIdentity
    ));
    fs::remove_file(&host)?;
    fs::hard_link(&visible, &host)?;
    identity.check(&visible, &host)?;
    Ok(())
}

#[test]
fn unproved_document_host_identity_refuses_existing_files_but_allows_fresh_outputs() {
    let root = tempfile::tempdir().expect("test root");
    let visible = root.path().join("visible-source.p4k");
    let host = root.path().join("unobservable-host/Data.p4k");
    let existing = root.path().join("outside-existing.txt");
    fs::write(&visible, b"source").expect("source");
    fs::write(&existing, b"prior output").expect("prior output");
    let identity =
        ArchiveSourceIdentity::documents(&visible, &host).expect("authenticated spelling");
    let refusal = identity
        .check(&existing, &existing)
        .expect_err("proxy inequality is not host inequality");
    assert!(
        matches!(refusal, WriteProtectionError::TargetIdentity { cause, .. } if cause.reason() == AliasUncertainty::HostFileIdentity)
    );
    let fresh = root.path().join("outside-new.txt");
    identity.check(&fresh, &fresh).expect("new distinct output");
    assert_eq!(fs::read(existing).expect("prior output"), b"prior output");
}

#[test]
fn matching_visible_identity_is_a_positive_overwrite_refusal_even_without_host_identity() {
    let root = tempfile::tempdir().expect("test root");
    let visible = root.path().join("visible-source.p4k");
    let alias = root.path().join("other-document.bin");
    fs::write(&visible, b"source").expect("source");
    fs::hard_link(&visible, &alias).expect("same visible object");
    let identity = ArchiveSourceIdentity::documents(&visible, &root.path().join("host/source.p4k"))
        .expect("source");
    assert!(matches!(
        identity.check(&alias, &root.path().join("host/alias.bin")),
        Err(WriteProtectionError::SourceOverwrite { .. })
    ));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_archive_namespace_distinguishes_case_spellings_from_distinct_links()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir()?;
    let original = directory.path().join("Data.p4k");
    let case_variant = directory.path().join("DATA.P4K");
    fs::write(&original, b"source")?;
    let identity = ArchiveSourceIdentity::new(&original, &original)?;
    if fs::exists(&case_variant)? {
        assert!(
            matches!(
                identity.check_namespace(&case_variant, &case_variant),
                Err(WriteProtectionError::SourceOverwrite { .. })
            ),
            "case aliases of the original directory entry are protected"
        );
    } else {
        fs::write(&case_variant, b"unrelated")?;
        identity.check_namespace(&case_variant, &case_variant)?;
    }

    let hard = directory.path().join("hardlink.p4k");
    let symbolic = directory.path().join("symlink.p4k");
    fs::hard_link(&original, &hard)?;
    symlink(&original, &symbolic)?;
    for alias in [hard, symbolic] {
        identity.check_namespace(&alias, &alias)?;
        assert!(
            matches!(
                identity.check(&alias, &alias),
                Err(WriteProtectionError::SourceOverwrite { .. })
            ),
            "the alias namespace is distinct but writing its contents would reach the archive"
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn macos_archive_root_replacement_respects_the_volume_case_rules()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir()?;
    let root = fs::canonicalize(directory.path())?;
    let containing = root.join("ArchiveFolder");
    let variant = root.join("ARCHIVEFOLDER");
    fs::create_dir(&containing)?;
    let archive = containing.join("Data.p4k");
    fs::write(&archive, b"source")?;
    let identity = ArchiveSourceIdentity::new(&archive, &archive)?;
    if fs::exists(&variant)? {
        assert!(
            matches!(
                identity.check_root(&variant, &variant),
                Err(WriteProtectionError::SourceOverwrite { .. })
            ),
            "replacement of a case alias must protect the contained Archive"
        );
    } else {
        fs::create_dir(&variant)?;
        identity.check_root(&variant, &variant)?;
    }

    let alias = root.join("folder-link");
    symlink(&containing, &alias)?;
    identity.check_root(&alias, &alias)?;
    Ok(())
}
