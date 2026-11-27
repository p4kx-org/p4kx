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

#[test]
fn selecting_only_a_leaf_use_keeps_its_original_enclosing_sources() -> TestResult {
    let (_directory, inner) = nested_leaf_archive()?;
    let origin = inner
        .entry_origin(inner.entries().first().ok_or("leaf Entry")?)
        .ok_or("actual leaf origin")?;
    let sources = SourceUses::with_compact_coordinates();
    let leaf = sources.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
    let outer = leaf.ancestor(0)?.ok_or("outer source")?;
    let middle = leaf.ancestor(1)?.ok_or("middle source")?;
    let used = leaf.at(
        SourceLocation::DataForge {
            record_index: 0,
            field: "field".into(),
        },
        SourceUseRole::Payload,
    )?;
    let _unselected = sources
        .observe(SourceInput::Unknown, InputScope::Payload)?
        .at(
            SourceLocation::Metadata {
                field: "other".into(),
            },
            SourceUseRole::Payload,
        )?;
    let completed = sources.finish_with(|| Ok::<(), crate::SourceEvidenceError>(()))?;
    let picked = completed.select_uses([used.reference()])?;
    let mut expected = [outer.source_ref(), middle.source_ref(), leaf.source_ref()];
    expected.sort_unstable();
    assert!(
        picked
            .iter_sources()
            .map(crate::PhysicalSource::id)
            .eq(expected),
        "only the leaf and its two original containers are retained",
    );
    assert!(
        picked
            .iter_uses()
            .map(crate::SourceUseDescription::id)
            .eq([used.reference()]),
        "the original leaf use remains the only use",
    );
    Ok(())
}

#[test]
fn marker_mode_uses_the_enclosing_archive_encryption_in_the_correct_byte_domain() -> TestResult {
    let (_directory, inner) = nested_leaf_archive()?;
    let origin = inner
        .entry_origin(inner.entries().first().ok_or("leaf Entry")?)
        .ok_or("actual leaf origin")?;
    let sources = SourceUses::with_provenance_mode(crate::ProvenanceMode::MarkersOnly);
    let leaf = sources.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
    let middle = leaf.ancestor(1)?.ok_or("encrypted container")?;
    assert_eq!(
        marker(&sources, &leaf, InputScope::Payload)?,
        Some(true),
        "a plain leaf value inside the encrypted container is marked"
    );
    assert_eq!(
        marker(&sources, &middle, InputScope::ArchiveDirectoryMetadata)?,
        Some(false),
        "the container's own directory entry is outside its ciphertext"
    );
    assert_eq!(
        sources.registered_use_count()?,
        0,
        "neither use needs a stored record"
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
