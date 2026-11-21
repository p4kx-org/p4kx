// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::ArchiveWriteProtection;
use crate::{DocumentGrant, HostAlias, PortalAlias, WriteProtectionError};

fn file_grant(host: &Path) -> ArchiveWriteProtection {
    ArchiveWriteProtection::from_alias(
        Path::new("/run/user/1000/doc/abc/Data.p4k"),
        PortalAlias::Resolved(HostAlias::new(
            host.to_path_buf(),
            "abc".into(),
            DocumentGrant::File,
        )),
    )
    .expect("authenticated file source")
}

#[test]
fn file_only_grant_refuses_parent_and_missing_descendants_without_a_protected_set() {
    let root = tempfile::tempdir().expect("root");
    let parent = root.path().join("unobservable-host-parent");
    let context = file_grant(&parent.join("Data.p4k"));
    for target in [&parent, &parent.join("absent/nested/file.bin")] {
        let failure = context
            .check_host(target, target)
            .expect_err("uncertain membership");
        assert!(matches!(
            failure,
            WriteProtectionError::SourceMembership { .. }
        ));
        assert!(failure.installation().is_none(), "no invented installation");
    }
    let outside = root.path().join("outside/new/file.bin");
    context
        .check_host(&outside, &outside)
        .expect("proven outside");
}

#[test]
fn visible_copy_is_not_classified_as_an_installation_by_its_filename() {
    let root = tempfile::tempdir().expect("root");
    std::fs::write(root.path().join("Data.p4k"), b"synthetic").expect("source");
    let host = root.path().join("Data.p4k");
    let context = ArchiveWriteProtection::from_alias(
        &host,
        PortalAlias::Resolved(HostAlias::new(
            host.clone(),
            "abc".into(),
            DocumentGrant::Directory,
        )),
    )
    .expect("directory source");
    let output = root.path().join("output.bin");
    context
        .check_host(&output, &output)
        .expect("ordinary copy parent");
}

#[test]
fn visible_library_children_are_protected_with_the_authenticated_host_parent() {
    let root = tempfile::tempdir().expect("root");
    std::fs::create_dir_all(root.path().join("PTU/Bin64")).expect("installation");
    let host = root.path().join("Data.p4k");
    let context = ArchiveWriteProtection::from_alias(
        &host,
        PortalAlias::Resolved(HostAlias::new(
            host.clone(),
            "abc".into(),
            DocumentGrant::Directory,
        )),
    )
    .expect("directory source");
    let output = root.path().join("PTU/new.bin");
    let failure = context
        .check_host(&output, &output)
        .expect_err("known installation");
    assert!(failure.installation().is_some());
}

#[test]
fn unavailable_directory_membership_remains_uncertain() {
    let root = tempfile::tempdir().expect("root");
    let parent = root.path().join("invisible");
    let host = parent.join("Data.p4k");
    let context = ArchiveWriteProtection::from_alias(
        Path::new("/invisible-document-parent/Data.p4k"),
        PortalAlias::Resolved(HostAlias::new(host, "abc".into(), DocumentGrant::Directory)),
    )
    .expect("authenticated source");
    let output = parent.join("new.bin");
    assert!(matches!(
        context.check_host(&output, &output),
        Err(WriteProtectionError::SourceMembership { .. })
    ));
}

#[test]
fn a_multiple_source_job_checks_the_second_origin_and_rejects_no_origin() {
    let root = tempfile::tempdir().expect("root");
    let ordinary = root.path().join("copy");
    let installation = root.path().join("PTU");
    std::fs::create_dir(&ordinary).expect("ordinary source parent");
    std::fs::create_dir_all(installation.join("Bin64")).expect("installation source parent");
    let first = ordinary.join("first.p4k");
    let second = installation.join("second.p4k");
    let cancel = AtomicBool::new(false);
    let context =
        ArchiveWriteProtection::for_sources(&[&first, &second], &cancel).expect("two sources");
    context
        .ensure_writable(&ordinary.join("report.txt"), &cancel)
        .expect("ordinary parent stays writable");
    let refused = context
        .ensure_writable(&installation.join("missing/report.txt"), &cancel)
        .expect_err("second source is protected");
    assert!(refused.installation().is_some());
    assert!(matches!(
        ArchiveWriteProtection::for_sources(&[], &cancel),
        Err(WriteProtectionError::MissingSource)
    ));
}

#[test]
fn ordinary_archive_and_its_hard_link_are_refused_without_blocking_the_parent() {
    let root = tempfile::tempdir().expect("root");
    let archive = root.path().join("Data.p4k");
    let alias = root.path().join("renamed-copy.bin");
    std::fs::write(&archive, b"synthetic Archive identity").expect("source");
    std::fs::hard_link(&archive, &alias).expect("hard link");
    let cancel = AtomicBool::new(false);
    let context = ArchiveWriteProtection::new(&archive, &cancel).expect("source protection");
    for target in [&archive, &alias] {
        let refused = context
            .ensure_writable(target, &cancel)
            .expect_err("input identity");
        assert!(matches!(
            refused,
            WriteProtectionError::SourceOverwrite { .. }
        ));
        assert!(refused.installation().is_none());
    }
    context
        .ensure_writable(&root.path().join("report.txt"), &cancel)
        .expect("ordinary sibling output");
}

#[test]
fn a_created_descriptor_proves_owned_content_without_inventing_a_native_portal_id() {
    use std::io::Write as _;
    use std::sync::atomic::Ordering;

    let root = tempfile::tempdir().expect("root");
    let visible = root.path().join("visible.p4k");
    std::fs::write(&visible, b"source").expect("source");
    let host = root.path().join("hidden/Data.p4k");
    let protection = ArchiveWriteProtection::from_alias(
        &visible,
        PortalAlias::Resolved(HostAlias::new(host, "abc".into(), DocumentGrant::File)),
    )
    .expect("source spelling");
    let cancel = AtomicBool::new(false);
    let path = root.path().join("new-output.bin");
    let (mut file, token) = crate::create_output_file(&path, Some(&protection), &cancel)
        .expect("exclusive output")
        .into_parts();
    file.write_all(b"owned output").expect("write");
    assert!(
        matches!(
            protection.ensure_writable(&path, &cancel),
            Err(WriteProtectionError::TargetIdentity { .. })
        ),
        "unowned checks retain the missing host proof"
    );
    token
        .ensure_writable(&path, &cancel)
        .expect("actual created descriptor");
    drop(file);
    cancel.store(true, Ordering::Release);
    token
        .remove(&path, &cancel)
        .expect("ordinary owned filename cleanup");
    assert!(!path.exists());
    assert_eq!(std::fs::read(visible).expect("source"), b"source");
}

#[test]
fn owned_namespace_cleanup_unlinks_hardlinks_without_changing_the_source() {
    use std::sync::atomic::Ordering;

    let root = tempfile::tempdir().expect("root");
    let visible = root.path().join("visible.p4k");
    std::fs::write(&visible, b"source").expect("source");
    let protection = ArchiveWriteProtection::from_alias(
        &visible,
        PortalAlias::Resolved(HostAlias::new(
            root.path().join("hidden/Data.p4k"),
            "abc".into(),
            DocumentGrant::File,
        )),
    )
    .expect("source spelling");
    let cancel = AtomicBool::new(false);
    let stage = root.path().join("owned-stage");
    let permit =
        crate::create_output_directory(&stage, Some(&protection), &cancel).expect("new stage");
    std::fs::hard_link(&visible, stage.join("source-alias")).expect("injected hardlink");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&visible, stage.join("source-link")).expect("injected symlink");
    cancel.store(true, Ordering::Release);
    permit.cleanup(&cancel).expect("namespace unlink");
    assert!(!stage.exists());
    assert_eq!(
        std::fs::read(visible).expect("original source name"),
        b"source"
    );
}

#[test]
fn file_token_rejects_retargeting_and_a_different_job() {
    let root = tempfile::tempdir().expect("root");
    let source = root.path().join("Data.p4k");
    std::fs::write(&source, b"source").expect("source");
    let cancel = AtomicBool::new(false);
    let protection = ArchiveWriteProtection::new(&source, &cancel).expect("job");
    let path = root.path().join("owned.bin");
    let (file, token) = crate::create_output_file(&path, Some(&protection), &cancel)
        .expect("new file")
        .into_parts();
    let different = ArchiveWriteProtection::new(&source, &cancel).expect("different job");
    assert!(different.ensure_owned_host(&path, &path, &token).is_err());
    drop(file);
    std::fs::remove_file(&path).expect("replace owned name");
    std::fs::hard_link(&source, &path).expect("source alias");
    assert!(token.ensure_writable(&path, &cancel).is_err());
    assert!(token.remove(&path, &cancel).is_err());
    assert_eq!(std::fs::read(source).expect("source"), b"source");
}

#[test]
fn replacement_namespace_cannot_contain_an_original_source_name() {
    let root = tempfile::tempdir().expect("root");
    let package = root.path().join("package");
    std::fs::create_dir(&package).expect("package");
    let source = package.join("input.p4k");
    std::fs::write(&source, b"source").expect("source");
    let cancel = AtomicBool::new(false);
    let protection = ArchiveWriteProtection::new(&source, &cancel).expect("job");
    let mut validated = false;
    let result = crate::validated_output_directory(&package, Some(&protection), &cancel, |_| {
        validated = true;
        Ok(())
    });
    assert!(result.is_err());
    assert!(
        !validated,
        "source containment is checked before domain adoption"
    );
    assert_eq!(std::fs::read(source).expect("source"), b"source");
}

#[cfg(unix)]
#[test]
fn a_source_symlink_retains_its_actual_installation_parent() {
    let root = tempfile::tempdir().expect("root");
    let game = root.path().join("PTU");
    std::fs::create_dir_all(game.join("Bin64")).expect("game layout");
    let source = game.join("Data.p4k");
    std::fs::write(&source, b"source").expect("source");
    let alias = root.path().join("archive-link");
    std::os::unix::fs::symlink(&source, &alias).expect("source alias");
    let cancel = AtomicBool::new(false);
    let protection = ArchiveWriteProtection::new(&alias, &cancel).expect("source identity");
    assert!(matches!(
        protection.ensure_writable(&game.join("output"), &cancel),
        Err(WriteProtectionError::Installation(_))
    ));
    protection
        .ensure_writable(&root.path().join("outside"), &cancel)
        .expect("outside remains writable");
}
