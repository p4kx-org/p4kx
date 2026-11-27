// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt as _;
use std::sync::atomic::AtomicBool;

use crate::portal_alias::{AliasUncertainty, PortalAlias};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn a_forged_host_path_xattr_on_an_ordinary_file_is_not_authority() -> TestResult {
    let root = tempfile::tempdir()?;
    let file = root.path().join("Data.p4k");
    fs::write(&file, b"fixture")?;
    rustix::fs::setxattr(
        &file,
        "user.document-portal.host-path",
        b"/private/LIVE/Data.p4k",
        rustix::fs::XattrFlags::empty(),
    )?;
    assert!(
        matches!(
            super::resolve_portal_alias(&file, &AtomicBool::new(false)),
            PortalAlias::NotPortal
        ),
        "an underlying user xattr does not turn an ordinary file into a document mapping"
    );
    Ok(())
}

#[test]
fn kernel_mount_inventory_is_bounded_and_preserves_read_failures() {
    struct Broken;
    impl io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "test mount input",
            ))
        }
    }
    let oversized = vec![b'x'; crate::portal_mount::MAX_MOUNT_BYTES + 1];
    assert_eq!(
        super::read_mounts(oversized.as_slice())
            .unwrap_err()
            .reason(),
        AliasUncertainty::Limit,
        "oversized kernel input is not partially trusted"
    );
    let error = super::read_mounts(Broken).unwrap_err();
    assert_eq!(
        error.reason(),
        AliasUncertainty::MountInventory,
        "an unreadable mount inventory does not imply absence"
    );
    assert!(
        Error::source(&error).is_some(),
        "the original local I/O failure remains available"
    );
}

#[test]
fn a_real_path_alias_into_a_document_mount_is_classified_after_resolution() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir_all(root.path().join("doc/aabbccdd/exports"))?;
    std::os::unix::fs::symlink(
        root.path().join("doc/aabbccdd/exports"),
        root.path().join("link"),
    )?;
    let mount_path = root.path().join("doc");
    let mut mounts = b"21 1 8:1 / / rw - ext4 /dev/root rw\n42 21 0:7 / ".to_vec();
    mounts.extend_from_slice(mount_path.as_os_str().as_bytes());
    mounts.extend_from_slice(b" rw - fuse.portal portal rw\n");
    let (path, _) =
        super::portal_target(&root.path().join("link/new/output"), &mounts, &mount_path)?
            .ok_or("portal alias was lost")?;
    assert_eq!(
        path,
        fs::canonicalize(root.path().join("doc/aabbccdd/exports"))?.join("new/output"),
        "a symlink outside the mount cannot hide a portal-backed write target"
    );
    Ok(())
}

#[test]
fn cancellation_is_unknown_even_with_an_empty_protection_set() {
    let installation = crate::Installation::new();
    assert!(
        installation.is_empty(),
        "a source-file-only grant need not expose an installation"
    );
    assert!(
        matches!(super::resolve_portal_alias(std::path::Path::new("/unseen/host/new"), &AtomicBool::new(true)), PortalAlias::Unknown(error) if error.reason() == AliasUncertainty::Cancelled),
        "the identity seam does not bypass uncertainty because protection discovery is empty"
    );
}
