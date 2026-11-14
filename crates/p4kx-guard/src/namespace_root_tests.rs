// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::fs;
use std::io;
use std::sync::atomic::AtomicBool;

use super::NamespaceRoot;
use crate::portal_path::{self, DocumentPath};
use crate::{AliasUncertainty, DocumentGrant, OwnedOutputError};

#[path = "namespace_root_fixture.rs"]
mod fixture;
#[path = "namespace_root_safety_tests.rs"]
mod safety;

use fixture::{Fixture, SOURCES, TestResult};

#[test]
fn a_grant_root_is_authenticated_directly_without_access_to_its_virtual_parent() -> TestResult {
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
        root.check_entry_with(&fixture.visible, &cancel, &lookup)?;
        let calls = calls.into_inner();
        assert_ne!(calls.len(), 0);
        assert!(calls.iter().all(|path| path == &fixture.visible));
    }
    Ok(())
}

#[test]
fn an_existing_descendant_directory_is_bound_to_the_same_source_context() -> TestResult {
    for source in SOURCES {
        let fixture = Fixture::new()?;
        let protection = fixture.protection(source)?;
        let cancel = AtomicBool::new(false);
        let child = fixture.visible.join("subdirectory");
        fs::create_dir(&child)?;
        let lookup = |path: &std::path::Path, cancel: &AtomicBool| fixture.lookup(path, cancel);
        let root = NamespaceRoot::bind_with(&child, Some(&protection), &cancel, &lookup)?;
        root.check_entry_with(&child.join("new.bin"), &cancel, &lookup)?;
        assert_eq!(root.host, fixture.host.join("subdirectory"));
    }
    Ok(())
}

#[test]
fn missing_children_are_authorized_without_inventing_a_parent_grant() -> TestResult {
    for source in SOURCES {
        let fixture = Fixture::new()?;
        let protection = fixture.protection(source)?;
        let cancel = AtomicBool::new(false);
        let lookup = |path: &std::path::Path, cancel: &AtomicBool| fixture.lookup(path, cancel);
        let root = NamespaceRoot::bind_with(&fixture.visible, Some(&protection), &cancel, &lookup)?;
        root.check_entry_with(&fixture.visible.join("new-stage"), &cancel, &lookup)?;
        root.check_entry_with(
            &fixture.visible.join("missing").join("leaf.bin"),
            &cancel,
            &lookup,
        )?;
        assert!(!fixture.visible.join("new-stage").exists());
        assert!(!fixture.visible.join("missing").exists());
    }
    Ok(())
}

#[test]
fn the_virtual_parent_and_sibling_names_never_gain_namespace_authority() -> TestResult {
    let fixture = Fixture::new()?;
    let cancel = AtomicBool::new(false);
    let lookup = |path: &std::path::Path, cancel: &AtomicBool| fixture.lookup(path, cancel);
    let root = NamespaceRoot::bind_with(&fixture.visible, None, &cancel, &lookup)?;
    let parent = fixture.visible.parent().ok_or("visible parent")?;
    for target in [
        parent.to_path_buf(),
        parent.join("Exports.part"),
        fixture.visible.join("..").join("escape"),
    ] {
        assert!(root.check_entry_with(&target, &cancel, &lookup).is_err());
    }
    assert!(lookup(parent, &cancel).is_err());
    Ok(())
}

#[test]
fn a_file_grant_root_cannot_be_bound_as_a_directory_namespace() -> TestResult {
    let fixture = Fixture::new()?;
    let file = fixture.visible.join("save.glb");
    fs::write(&file, b"existing selected file")?;
    let cancel = AtomicBool::new(false);
    let lookup = |path: &std::path::Path, cancel: &AtomicBool| fixture.lookup(path, cancel);
    assert!(matches!(
        NamespaceRoot::bind_with(&file, None, &cancel, &lookup),
        Err(OwnedOutputError::Protection(_))
    ));
    assert_eq!(fs::read(file)?, b"existing selected file");
    Ok(())
}

#[test]
fn a_nonexistent_savefile_root_stays_an_original_filesystem_error() -> TestResult {
    let fixture = Fixture::new()?;
    let file = fixture.visible.join("new-save.glb");
    let cancel = AtomicBool::new(false);
    let lookup = |path: &std::path::Path, cancel: &AtomicBool| fixture.lookup(path, cancel);
    let expected = fs::symlink_metadata(&file)
        .err()
        .ok_or("absent save target")?;
    let error = NamespaceRoot::bind_with(&file, None, &cancel, &lookup)
        .err()
        .ok_or("absent root authorized")?;
    let OwnedOutputError::Io { source, .. } = error else {
        return Err("filesystem cause changed".into());
    };
    assert_eq!(source.kind(), io::ErrorKind::NotFound);
    assert_eq!(source.raw_os_error(), expected.raw_os_error());
    assert!(!file.exists());
    Ok(())
}

#[test]
fn virtual_file_siblings_and_file_descendants_are_not_guessed_as_host_names() -> TestResult {
    let sibling = DocumentPath {
        id: "abcd".into(),
        name: b"save.glb.part".to_vec(),
        suffix: Vec::new(),
    };
    assert_eq!(
        portal_path::mapped_descendant(&sibling, b"/host/save.glb\0", DocumentGrant::File)
            .err()
            .ok_or("sibling mapped")?
            .reason(),
        AliasUncertainty::MalformedReply
    );
    let descendant = DocumentPath {
        id: "abcd".into(),
        name: b"save.glb".to_vec(),
        suffix: vec![b"child".to_vec()],
    };
    assert_eq!(
        portal_path::mapped_descendant(&descendant, b"/host/save.glb\0", DocumentGrant::File)
            .err()
            .ok_or("file descendant mapped")?
            .reason(),
        AliasUncertainty::FileDescendant
    );
    Ok(())
}
