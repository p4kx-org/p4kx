// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::portal_alias::AliasUncertainty;

const TABLE: &[u8] = b"21 1 8:1 / / rw - ext4 /dev/root rw\n42 21 0:7 / /run/user/1001/doc rw shared:7 - fuse.portal portal rw\n";

#[test]
fn covering_mount_uses_component_boundaries_and_the_deepest_mount() {
    let selected = super::covering_mount(TABLE, b"/run/user/1001/doc/aabbccdd/export")
        .unwrap()
        .unwrap();
    assert!(
        selected.is_documents(),
        "the exact portal mount covers its descendants"
    );
    assert!(
        !super::covering_mount(TABLE, b"/run/user/1001/document/export")
            .unwrap()
            .unwrap()
            .is_documents(),
        "a string prefix is not a mount boundary"
    );
    let mut nested = TABLE.to_vec();
    nested
        .extend_from_slice(b"43 42 0:8 / /run/user/1001/doc/aabbccdd/export rw - tmpfs tmpfs rw\n");
    assert!(
        !super::covering_mount(&nested, b"/run/user/1001/doc/aabbccdd/export/new")
            .unwrap()
            .unwrap()
            .is_documents(),
        "a nested foreign mount overrides the portal prefix"
    );
}

#[test]
fn mount_identity_changes_remain_observable() {
    let path = b"/run/user/1001/doc/aabbccdd/export";
    let original = super::covering_mount(TABLE, path).unwrap();
    for changed in [
        String::from_utf8(TABLE.to_vec())
            .unwrap()
            .replace("42 21", "43 21"),
        String::from_utf8(TABLE.to_vec())
            .unwrap()
            .replace("0:7", "0:9"),
        String::from_utf8(TABLE.to_vec())
            .unwrap()
            .replace("shared:7", "shared:8"),
    ] {
        assert_ne!(
            original,
            super::covering_mount(changed.as_bytes(), path).unwrap(),
            "replaced mounts cannot reuse only a display spelling as identity"
        );
    }
    let duplicate = [
        TABLE,
        b"42 21 0:7 / /run/user/1001/doc rw - fuse.portal portal rw\n",
    ]
    .concat();
    assert_eq!(
        super::covering_mount(&duplicate, path)
            .unwrap_err()
            .reason(),
        AliasUncertainty::UntrustedMount,
        "equal covering mount points are ambiguous"
    );
}

#[test]
fn kernel_escapes_and_truncated_inventory_are_distinct() {
    let mount = super::covering_mount(
        b"42 21 0:7 / /portal\\040name rw - fuse.portal portal rw\n",
        b"/portal name/aabbccdd/export",
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        mount.target, b"/portal name",
        "kernel octal escapes preserve the actual pathname bytes"
    );
    for invalid in [
        b"42 21 0:7 / /portal rw - fuse.portal portal".as_slice(),
        b"42 21 0:7 / /portal rw fuse.portal portal rw",
        b"42 21 0:7 / /portal\\04 rw - fuse.portal portal rw",
        b"42 21 0:7 / /portal rw - fuse.portal portal rw unexpected",
    ] {
        assert_eq!(
            super::covering_mount(invalid, b"/portal/aabbccdd/export")
                .unwrap_err()
                .reason(),
            AliasUncertainty::MountInventory,
            "truncated or malformed kernel metadata is never a trusted negative"
        );
    }
}
