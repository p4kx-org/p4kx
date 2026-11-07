// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::ArchiveSourceIdentity;
use crate::{AliasUncertainty, WriteProtectionError};

#[test]
fn unproved_document_host_identity_refuses_existing_files_but_allows_fresh_outputs() {
    let root = tempfile::tempdir().expect("test root");
    let visible = root.path().join("visible-source.p4k");
    let host = root.path().join("unobservable-host/Data.p4k");
    let existing = root.path().join("outside-existing.txt");
    std::fs::write(&visible, b"source").expect("source");
    std::fs::write(&existing, b"prior output").expect("prior output");
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
    assert_eq!(
        std::fs::read(existing).expect("prior output"),
        b"prior output"
    );
}

#[test]
fn matching_visible_identity_is_a_positive_overwrite_refusal_even_without_host_identity() {
    let root = tempfile::tempdir().expect("test root");
    let visible = root.path().join("visible-source.p4k");
    let alias = root.path().join("other-document.bin");
    std::fs::write(&visible, b"source").expect("source");
    std::fs::hard_link(&visible, &alias).expect("same visible object");
    let identity = ArchiveSourceIdentity::documents(&visible, &root.path().join("host/source.p4k"))
        .expect("source");
    assert!(matches!(
        identity.check(&alias, &root.path().join("host/alias.bin")),
        Err(WriteProtectionError::SourceOverwrite { .. })
    ));
}
