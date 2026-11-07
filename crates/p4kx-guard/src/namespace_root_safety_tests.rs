// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};

use super::fixture::{self, Fixture, SOURCES, SourceKind, TestResult};
use crate::namespace_root::{NamespaceRoot, RootPresence};
use crate::{
    AliasUncertainty, ArchiveWriteProtection, OwnedOutputCleanupError, OwnedOutputError,
    WriteProtectionError, owned_output_cleanup,
};

#[test]
fn changed_root_identity_retains_the_owned_tree_and_foreign_replacement() -> TestResult {
    for source in SOURCES {
        let fixture = Fixture::new()?;
        let protection = fixture.protection(source)?;
        let cancel = AtomicBool::new(false);
        let lookup = |path: &std::path::Path, cancel: &AtomicBool| fixture.lookup(path, cancel);
        let root = NamespaceRoot::bind_with(&fixture.visible, Some(&protection), &cancel, &lookup)?;
        fs::write(fixture.visible.join("owned"), b"owned bytes")?;
        let retained = fixture.folder.path().join("retained-root");
        fs::rename(&fixture.visible, &retained)?;
        fs::create_dir(&fixture.visible)?;
        fs::write(fixture.visible.join("foreign"), b"foreign bytes")?;
        let result = owned_output_cleanup::cleanup_with(
            root.path(),
            &|| root.revalidate_with(&cancel, RootPresence::AbsentAllowed, &lookup),
            &|path| root.check_entry_with(path, &cancel, &lookup),
        );
        assert!(matches!(
            result,
            Err(OwnedOutputCleanupError::Protection(_))
        ));
        assert_eq!(fs::read(fixture.visible.join("foreign"))?, b"foreign bytes");
        assert_eq!(fs::read(retained.join("owned"))?, b"owned bytes");
    }
    Ok(())
}

#[test]
fn changed_host_mapping_mount_or_service_owner_never_reuses_old_authentication() -> TestResult {
    let fixture = Fixture::new()?;
    let cancel = AtomicBool::new(false);
    let root = NamespaceRoot::bind_with(&fixture.visible, None, &cancel, &|path, cancel| {
        fixture.lookup(path, cancel)
    })?;
    let changed = root.revalidate_with(&cancel, RootPresence::Present, &|_, _| {
        Ok(fixture.folder.path().join("different-host"))
    });
    assert!(changed.is_err());
    for reason in [
        AliasUncertainty::OwnerChanged,
        AliasUncertainty::UntrustedMount,
    ] {
        let error = root
            .revalidate_with(&cancel, RootPresence::Present, &|path, _| {
                Err(fixture::refused(path, reason))
            })
            .err()
            .ok_or("stale root accepted")?;
        assert!(
            matches!(error, WriteProtectionError::TargetIdentity { cause, .. } if cause.reason() == reason)
        );
    }
    Ok(())
}

#[test]
fn portal_cleanup_honours_the_actual_cancel_flag_and_retains_its_cause() -> TestResult {
    let fixture = Fixture::new()?;
    let cancel = AtomicBool::new(false);
    let lookup = |path: &std::path::Path, cancel: &AtomicBool| fixture.lookup(path, cancel);
    let root = NamespaceRoot::bind_with(&fixture.visible, None, &cancel, &lookup)?;
    fs::write(fixture.visible.join("owned"), b"owned bytes")?;
    cancel.store(true, Ordering::Release);
    let result = owned_output_cleanup::cleanup_with(
        root.path(),
        &|| root.revalidate_with(&cancel, RootPresence::AbsentAllowed, &lookup),
        &|path| root.check_entry_with(path, &cancel, &lookup),
    );
    let error = result
        .err()
        .ok_or("cancelled portal authentication succeeded")?;
    assert!(error.is_cancelled());
    assert_eq!(fs::read(fixture.visible.join("owned"))?, b"owned bytes");
    Ok(())
}

#[test]
fn cleanup_rechecks_the_bound_root_without_authorising_its_virtual_ancestors() -> TestResult {
    for source in SOURCES {
        let fixture = Fixture::new()?;
        let protection = fixture.protection(source)?;
        let cancel = AtomicBool::new(false);
        let calls = RefCell::new(Vec::new());
        let lookup = |path: &std::path::Path, cancel: &AtomicBool| {
            calls.borrow_mut().push(path.to_path_buf());
            fixture.lookup(path, cancel)
        };
        let root = NamespaceRoot::bind_with(&fixture.visible, Some(&protection), &cancel, &lookup)?;
        fs::create_dir(fixture.visible.join("sub"))?;
        fs::write(fixture.visible.join("sub").join("owned"), b"owned bytes")?;
        owned_output_cleanup::cleanup_with(
            root.path(),
            &|| root.revalidate_with(&cancel, RootPresence::AbsentAllowed, &lookup),
            &|path| root.check_entry_with(path, &cancel, &lookup),
        )?;
        assert!(!fixture.visible.exists());
        assert!(
            calls
                .borrow()
                .iter()
                .all(|path| path.starts_with(&fixture.visible))
        );
        assert_eq!(
            fs::read(fixture.source_for(source))?,
            b"synthetic source bytes"
        );
    }
    Ok(())
}

#[test]
fn a_source_hardlink_can_only_be_unlinked_inside_the_owned_namespace() -> TestResult {
    for source in SOURCES {
        let fixture = Fixture::new()?;
        let protection = fixture.protection(source)?;
        let cancel = AtomicBool::new(false);
        let lookup = |path: &std::path::Path, cancel: &AtomicBool| fixture.lookup(path, cancel);
        let root = NamespaceRoot::bind_with(&fixture.visible, Some(&protection), &cancel, &lookup)?;
        let linked = fixture.visible.join("source-link");
        fs::hard_link(fixture.source_for(source), &linked)?;
        root.check_entry_with(&linked, &cancel, &lookup)?;
        assert!(matches!(
            protection.ensure_writable(&linked, &cancel),
            Err(WriteProtectionError::SourceOverwrite { .. })
        ));
        owned_output_cleanup::cleanup_with(
            root.path(),
            &|| root.revalidate_with(&cancel, RootPresence::AbsentAllowed, &lookup),
            &|path| root.check_entry_with(path, &cancel, &lookup),
        )?;
        assert!(!linked.exists());
        assert_eq!(
            fs::read(fixture.source_for(source))?,
            b"synthetic source bytes"
        );
    }
    Ok(())
}

#[test]
fn source_containment_and_unproved_source_parent_membership_still_refuse_roots() -> TestResult {
    let fixture = Fixture::new()?;
    let cancel = AtomicBool::new(false);
    let ordinary = fixture.protection(SourceKind::Ordinary)?;
    let source_parent = fs::canonicalize(fixture.folder.path())?;
    let result = NamespaceRoot::bind_with(&fixture.visible, Some(&ordinary), &cancel, &|_, _| {
        Ok(source_parent.clone())
    });
    assert!(matches!(
        result,
        Err(OwnedOutputError::Protection(
            WriteProtectionError::SourceOverwrite { .. }
        ))
    ));
    let file_grant = fixture.protection(SourceKind::FileGrant)?;
    let hidden_parent = fixture.host_source.parent().ok_or("hidden parent")?;
    let result = NamespaceRoot::bind_with(&fixture.visible, Some(&file_grant), &cancel, &|_, _| {
        Ok(hidden_parent.join("output"))
    });
    assert!(matches!(
        result,
        Err(OwnedOutputError::Protection(
            WriteProtectionError::SourceMembership { .. }
        ))
    ));
    assert!(matches!(
        crate::namespace_target::check_host(&fixture.source, &fixture.source, Some(&ordinary)),
        Err(WriteProtectionError::SourceOverwrite { .. })
    ));
    Ok(())
}

#[test]
fn a_directory_namespace_does_not_authorise_an_unowned_fuse_file_content_write() -> TestResult {
    let fixture = Fixture::new()?;
    let protection = fixture.protection(SourceKind::FileGrant)?;
    let cancel = AtomicBool::new(false);
    let lookup = |path: &std::path::Path, cancel: &AtomicBool| fixture.lookup(path, cancel);
    let root = NamespaceRoot::bind_with(&fixture.visible, Some(&protection), &cancel, &lookup)?;
    let existing = fixture.visible.join("unowned-file");
    fs::write(&existing, b"foreign bytes")?;
    root.check_entry_with(&existing, &cancel, &lookup)?;
    let error = protection
        .ensure_writable(&existing, &cancel)
        .err()
        .ok_or("unknown host file authorised")?;
    assert!(
        matches!(error, WriteProtectionError::TargetIdentity { cause, .. }
        if cause.reason() == AliasUncertainty::HostFileIdentity)
    );
    assert_eq!(fs::read(existing)?, b"foreign bytes");
    Ok(())
}

#[test]
fn an_established_installation_is_not_overridden_by_directory_mapping() -> TestResult {
    let fixture = Fixture::new()?;
    fs::create_dir(fixture.folder.path().join("Bin64"))?;
    let protection = fixture.protection(SourceKind::Ordinary)?;
    let cancel = AtomicBool::new(false);
    let result = NamespaceRoot::bind_with(
        &fixture.visible,
        Some(&protection),
        &cancel,
        &|path, cancel| fixture.lookup(path, cancel),
    );
    assert!(matches!(
        result,
        Err(OwnedOutputError::Protection(
            WriteProtectionError::Installation(_)
        ))
    ));
    Ok(())
}

#[test]
fn ordinary_created_and_validated_roots_use_the_real_protection_and_cancel_flag() -> TestResult {
    let fixture = Fixture::new()?;
    let cancel = AtomicBool::new(false);
    let protection = ArchiveWriteProtection::new(&fixture.source, &cancel)?;
    let stage = fixture.folder.path().join("real-stage");
    let permit = crate::create_output_directory(&stage, Some(&protection), &cancel)?;
    fs::write(stage.join("owned"), b"owned bytes")?;
    let checked = crate::validated_output_directory(&stage, Some(&protection), &cancel, |root| {
        assert_eq!(fs::read(root.join("owned"))?, b"owned bytes");
        Ok(())
    })?;
    checked.ensure_namespace_writable(&stage.join("owned"), &cancel)?;
    drop(checked);
    cancel.store(true, Ordering::Release);
    permit.cleanup(&cancel)?;
    assert!(!stage.exists());
    assert_eq!(fs::read(fixture.source)?, b"synthetic source bytes");
    Ok(())
}

#[test]
fn directory_creation_rejects_parent_traversal_before_creating_the_resolved_target() -> TestResult {
    let fixture = Fixture::new()?;
    let existing = fixture.folder.path().join("existing-parent");
    fs::create_dir(&existing)?;
    let target = existing.join("..").join("must-not-exist");
    let cancel = AtomicBool::new(false);
    let protection = fixture.protection(SourceKind::Ordinary)?;
    assert!(crate::create_output_directory(&target, Some(&protection), &cancel).is_err());
    assert!(!fixture.folder.path().join("must-not-exist").exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn an_existing_symbolic_directory_is_never_adopted_as_the_owned_root() -> TestResult {
    let fixture = Fixture::new()?;
    let link = fixture.visible.join("link");
    std::os::unix::fs::symlink(&fixture.host, &link)?;
    let cancel = AtomicBool::new(false);
    let lookup = |path: &std::path::Path, cancel: &AtomicBool| fixture.lookup(path, cancel);
    assert!(NamespaceRoot::bind_with(&link, None, &cancel, &lookup).is_err());
    assert!(fixture.host.is_dir());
    Ok(())
}
