// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::symlink;

use crate::portal_alias::{AliasUncertainty, DocumentGrant};
use crate::portal_path::DocumentPath;

type TestResult = Result<(), Box<dyn Error>>;

fn document(name: &[u8], suffix: &[&[u8]]) -> DocumentPath {
    DocumentPath {
        id: "aabbccdd".to_owned(),
        name: name.to_vec(),
        suffix: suffix.iter().map(|part| part.to_vec()).collect(),
    }
}

#[test]
fn a_file_only_grant_never_inspects_hidden_host_siblings() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("aabbccdd"))?;
    fs::write(root.path().join("aabbccdd/Data.p4k"), b"fixture")?;
    let snapshot = super::GrantSnapshot::capture(
        root.path().as_os_str().as_bytes(),
        &document(b"Data.p4k", &[]),
    )?;
    assert_eq!(
        snapshot.grant,
        DocumentGrant::File,
        "the portal view supplies file kind without access to Bin64 or the host parent"
    );
    assert_eq!(
        snapshot.objects.len(),
        1,
        "no host sibling discovery is inferred from a Data.p4k basename"
    );
    assert_eq!(
        super::GrantSnapshot::capture(
            root.path().as_os_str().as_bytes(),
            &document(b"Data.p4k", &[b"new"])
        )
        .unwrap_err()
        .reason(),
        AliasUncertainty::FileDescendant,
        "a file grant cannot own a new output subtree"
    );
    Ok(())
}

#[test]
fn missing_output_suffixes_are_mapped_without_creating_them() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir_all(root.path().join("aabbccdd/exports"))?;
    let selected = document(b"exports", &[b"new", b"deeper", b"output.bin"]);
    let before = super::GrantSnapshot::capture(root.path().as_os_str().as_bytes(), &selected)?;
    assert_eq!(
        before.grant,
        DocumentGrant::Directory,
        "a directory grant can own an absent descendant"
    );
    assert!(
        !root.path().join("aabbccdd/exports/new").exists(),
        "identity capture performs no writes"
    );
    fs::create_dir(root.path().join("aabbccdd/exports/new"))?;
    assert_ne!(
        before,
        super::GrantSnapshot::capture(root.path().as_os_str().as_bytes(), &selected)?,
        "new filesystem objects invalidate a previously captured mapping"
    );
    Ok(())
}

#[test]
fn symlink_descendants_and_symlink_grant_roots_are_uncertain() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir_all(root.path().join("aabbccdd/exports"))?;
    fs::create_dir(root.path().join("elsewhere"))?;
    symlink(
        root.path().join("elsewhere"),
        root.path().join("aabbccdd/exports/link"),
    )?;
    let selected = document(b"exports", &[b"link", b"new"]);
    assert_eq!(
        super::GrantSnapshot::capture(root.path().as_os_str().as_bytes(), &selected)
            .unwrap_err()
            .reason(),
        AliasUncertainty::SymbolicLink,
        "a lexical suffix cannot authenticate a symlink destination"
    );
    symlink(
        root.path().join("elsewhere"),
        root.path().join("aabbccdd/root-link"),
    )?;
    assert_eq!(
        super::GrantSnapshot::capture(
            root.path().as_os_str().as_bytes(),
            &document(b"root-link", &[])
        )
        .unwrap_err()
        .reason(),
        AliasUncertainty::SymbolicLink,
        "the exposed object itself must have an observable non-link identity"
    );
    Ok(())
}

#[test]
fn a_dangling_or_inaccessible_ancestor_is_not_a_proven_path() -> TestResult {
    let root = tempfile::tempdir()?;
    symlink(root.path().join("absent"), root.path().join("dangling"))?;
    assert_eq!(
        super::existing_ancestor(&root.path().join("dangling/../new"))
            .unwrap_err()
            .reason(),
        AliasUncertainty::DocumentPath,
        "an unresolved parent component is not appended to a guessed ancestor"
    );
    let visible = root.path().join("visible");
    fs::create_dir(&visible)?;
    let resolved = super::existing_ancestor(&visible.join("new/deeper"))?;
    assert_eq!(
        resolved,
        fs::canonicalize(&visible)?.join("new/deeper"),
        "only normal absent components are appended to the observed ancestor"
    );
    Ok(())
}
