// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use tempfile::TempDir;

use crate::{
    Archive, InputScope, SourceInput, SourceLocation, SourceScope, SourceUseRole, SourceUses,
    SourceValue,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn existing_ancestor_scopes_keep_only_their_original_enclosing_encryption() -> TestResult {
    let (_directory, inner) = nested_leaf_archive()?;
    let origin = inner
        .entry_origin(inner.entries().first().ok_or("leaf Entry")?)
        .ok_or("actual leaf origin")?;
    let sources = SourceUses::new();
    let payload = sources.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
    let outer_scope = payload.ancestor(0)?.ok_or("outer original scope")?;
    let inner_scope = payload.ancestor(1)?.ok_or("inner original scope")?;
    assert!(
        payload.ancestor(2)?.is_none(),
        "the leaf is not its own ancestor"
    );
    assert!(
        payload.ancestor(usize::MAX)?.is_none(),
        "an out-of-range position cannot resolve an arbitrary reference"
    );
    assert_eq!(
        marker(&sources, &outer_scope, InputScope::ArchiveDirectoryMetadata)?,
        Some(false),
        "later encrypted Entries do not taint the root directory"
    );
    assert_eq!(
        marker(&sources, &inner_scope, InputScope::ArchiveDirectoryMetadata)?,
        Some(false),
        "inner directory metadata lives in the plain outer Entry"
    );
    assert_eq!(
        marker(&sources, &inner_scope, InputScope::Payload)?,
        Some(true),
        "the encrypted inner Entry retains its own payload declaration"
    );
    assert_eq!(
        marker(&sources, &payload, InputScope::ArchiveDirectoryMetadata)?,
        Some(true),
        "the leaf directory lives inside the encrypted inner Entry"
    );
    assert_eq!(
        sources.freeze()?.sources().len(),
        3,
        "ancestor getters reuse the existing original physical rows"
    );
    let foreign = SourceUses::new().value(
        SourceValue::Present(()),
        &[outer_scope.at(
            SourceLocation::Metadata {
                field: "path".to_owned(),
            },
            SourceUseRole::Payload,
        )?],
        true,
    );
    assert!(
        !foreign.origin().source_set_complete(),
        "the original inventory owner remains sealed"
    );
    assert!(
        foreign.origin().source_uses_refs().is_empty(),
        "equal-looking wire IDs cannot grant a different owner authority"
    );
    Ok(())
}

fn marker(
    sources: &SourceUses,
    scope: &SourceScope,
    domain: InputScope,
) -> Result<Option<bool>, crate::SourceEvidenceError> {
    let handle = scope.for_scope(domain).at(
        SourceLocation::Metadata {
            field: "fact".to_owned(),
        },
        SourceUseRole::Conversion,
    )?;
    Ok(sources
        .value(SourceValue::Present(()), &[handle], true)
        .origin()
        .contains_encrypted_source())
}

fn nested_leaf_archive() -> TestResult<(TempDir, Archive)> {
    let leaf = ArchiveBuilder::new().entry(TestEntry::stored("leaf.ext", b"leaf"));
    let middle =
        ArchiveBuilder::new().entry(TestEntry::deflated("inner.pak", leaf.build()).encrypted());
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("outer.pak", middle.build()))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    archive.set_key(test_key());
    let outer = archive.open_container(archive.entries().first().ok_or("outer Entry")?)?;
    let inner = outer.open_container(outer.entries().first().ok_or("inner Entry")?)?;
    Ok((directory, inner))
}
