// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original middle and root prefix observation-order cases.

use p4kx_extraction::{Archive, SourceDetails, SourceScope, SourceUse, SourceUses};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::source_minimal_proof as support;
use crate::source_minimal_proof::TestResult;

use super::checks;

pub(super) fn shared_unknown_prefix_upgrades_one_original_ancestor(
    deeper_first: bool,
) -> TestResult {
    let leaf = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.bin", b"same"))
        .build();
    let middle_bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("inner.pak", leaf))
        .build();
    let (_directory, root) =
        support::open(&ArchiveBuilder::new().entry(TestEntry::stored("outer.pak", middle_bytes)))?;
    let detached = support::first(&root)?.clone();
    let middle = root.open_container(&detached)?;
    let deeper = middle.open_container(support::first(&middle)?)?;
    let sources = SourceUses::new();
    let (separate, deep) = if deeper_first {
        let deep = support::source(&sources, &deeper, SourceDetails::CaptureLimited)?;
        let separate = support::source(&sources, &middle, SourceDetails::CaptureLimited)?;
        (separate, deep)
    } else {
        let separate = support::source(&sources, &middle, SourceDetails::CaptureLimited)?;
        let deep = support::source(&sources, &deeper, SourceDetails::CaptureLimited)?;
        (separate, deep)
    };
    let ancestor = deep.ancestor(1)?.ok_or("original middle ancestor")?;
    let inherited_use = shared_middle(&sources, &ancestor, &separate, &middle, deeper_first)?;
    upgraded_middle(&sources, &ancestor, &middle, &inherited_use)?;
    Ok(())
}

pub(super) fn known_prefix_before_an_unknown_view_upgrades_one_root(
    deeper_first: bool,
) -> TestResult {
    let leaf = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.bin", b"same"))
        .build();
    let outer_bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("inner.pak", leaf))
        .build();
    let (_directory, root) =
        support::open(&ArchiveBuilder::new().entry(TestEntry::stored("outer.pak", outer_bytes)))?;
    let outer = root.open_container(support::first(&root)?)?;
    let detached = support::first(&outer)?.clone();
    let deeper = outer.open_container(&detached)?;
    let sources = SourceUses::new();
    let (separate, deep) = if deeper_first {
        let deep = support::source(&sources, &deeper, SourceDetails::CaptureLimited)?;
        let separate = support::source(&sources, &root, SourceDetails::CaptureLimited)?;
        (separate, deep)
    } else {
        let separate = support::source(&sources, &root, SourceDetails::CaptureLimited)?;
        let deep = support::source(&sources, &deeper, SourceDetails::CaptureLimited)?;
        (separate, deep)
    };
    let ancestor = deep.ancestor(0)?.ok_or("original root ancestor")?;
    assert_eq!(
        ancestor.source_ref(),
        separate.source_ref(),
        "a proved root prefix cannot inherit an unrelated deeper-view nonce; deeper_first={deeper_first}"
    );
    let inherited_use = support::used(&ancestor)?;
    assert_eq!(
        inherited_use.reference(),
        support::used(&separate)?.reference(),
        "the original root coordinate shares its use reference"
    );
    upgraded_root(&sources, &ancestor, &root, &inherited_use)?;
    Ok(())
}

pub(super) fn legacy_full_descendants_and_separate_roots_keep_existing_refs(
    root_first: bool,
) -> TestResult {
    let leaf = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.bin", b"same"))
        .build();
    let outer_bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("inner.pak", leaf))
        .build();
    let (_directory, root) =
        support::open(&ArchiveBuilder::new().entry(TestEntry::stored("outer.pak", outer_bytes)))?;
    let outer = root.open_container(support::first(&root)?)?;
    let detached = support::first(&outer)?.clone();
    let deeper = outer.open_container(&detached)?;
    let sources = SourceUses::new();
    let (separate, deep) = if root_first {
        let separate = checks::full_source(&sources, &root)?;
        let deep = checks::full_source(&sources, &deeper)?;
        (separate, deep)
    } else {
        let deep = checks::full_source(&sources, &deeper)?;
        let separate = checks::full_source(&sources, &root)?;
        (separate, deep)
    };
    let ancestor = deep.ancestor(0)?.ok_or("original root ancestor")?;
    assert_eq!(
        ancestor.source_ref(),
        separate.source_ref(),
        "the first registered root ID remains authoritative in either legacy order; root_first={root_first}"
    );
    let original_use = support::used(&ancestor)?;
    assert_eq!(
        original_use.reference(),
        support::used(&separate)?.reference(),
        "legacy ancestor and separate observations share the actual coordinate"
    );
    selected_legacy_root(&sources, &root, &deeper, &deep, &ancestor, &original_use)?;
    Ok(())
}

fn shared_middle(
    sources: &SourceUses,
    ancestor: &SourceScope,
    separate: &SourceScope,
    middle: &Archive,
    deeper_first: bool,
) -> TestResult<SourceUse> {
    assert_eq!(
        ancestor.source_ref(),
        separate.source_ref(),
        "the same original middle Entry keeps its ID across view depths; deeper_first={deeper_first}"
    );
    let inherited_use = support::used(ancestor)?;
    let direct_use = support::used(separate)?;
    assert_eq!(
        inherited_use.reference(),
        direct_use.reference(),
        "same original coordinate retains one use reference"
    );
    assert!(
        ancestor.matches_entry(
            &middle
                .observed_entry(support::first(middle)?)
                .ok_or("actual middle view")?
        )?,
        "the shared prefix retains its actual original binding"
    );
    let wire = serde_json::to_value(sources.freeze()?)?;
    assert_eq!(
        checks::physical_rows(&wire)?.len(),
        3,
        "one unknown prefix, one middle Entry and one leaf survive either order"
    );
    assert_eq!(
        checks::row(&wire, ancestor)?
            .get("kind")
            .ok_or("retained original source row")?,
        "archiveEntryProof",
        "the actual ancestor begins with admitted proof only"
    );
    Ok(inherited_use)
}

fn upgraded_middle(
    sources: &SourceUses,
    ancestor: &SourceScope,
    middle: &Archive,
    inherited_use: &SourceUse,
) -> TestResult {
    let full = checks::full_source(sources, middle)?;
    assert_eq!(
        full.source_ref(),
        ancestor.source_ref(),
        "Full upgrades the original row referenced by the deeper scope"
    );
    assert_eq!(
        support::used(ancestor)?.reference(),
        inherited_use.reference(),
        "an existing ancestor scope keeps its use after Full upgrade"
    );
    let upgraded = serde_json::to_value(sources.freeze()?)?;
    assert_eq!(
        checks::physical_rows(&upgraded)?.len(),
        3,
        "Full upgrade adds no physical source"
    );
    assert_eq!(
        checks::row(&upgraded, ancestor)?
            .get("kind")
            .ok_or("retained original source row")?,
        "archiveEntry",
        "the earlier ancestor scope now resolves to complete directory details"
    );
    assert_eq!(
        checks::row(&upgraded, ancestor)?
            .get("path")
            .and_then(|path| path.get("value"))
            .ok_or("retained original source row")?,
        "inner.pak",
        "Full details belong to the original middle Entry"
    );
    let full_bytes = serde_json::to_vec(&sources.freeze()?)?;
    let limited = support::source(sources, middle, SourceDetails::CaptureLimited)?;
    assert_eq!(
        limited.source_ref(),
        ancestor.source_ref(),
        "later limited capture keeps the upgraded prefix"
    );
    assert_eq!(
        serde_json::to_vec(&sources.freeze()?)?,
        full_bytes,
        "limited capture neither downgrades nor duplicates a Full prefix"
    );
    support::assert_closed(&sources.freeze()?)?;
    Ok(())
}

fn upgraded_root(
    sources: &SourceUses,
    ancestor: &SourceScope,
    root: &Archive,
    inherited_use: &SourceUse,
) -> TestResult {
    let full = checks::full_source(sources, root)?;
    assert_eq!(
        full.source_ref(),
        ancestor.source_ref(),
        "separate Full observation upgrades the original root row"
    );
    assert_eq!(
        support::used(ancestor)?.reference(),
        inherited_use.reference(),
        "earlier descendant references remain valid after upgrade"
    );
    let wire = serde_json::to_value(sources.freeze()?)?;
    assert_eq!(
        checks::physical_rows(&wire)?.len(),
        3,
        "the known root is retained once alongside the unknown step and leaf"
    );
    assert_eq!(
        checks::row(&wire, ancestor)?
            .get("kind")
            .ok_or("retained original source row")?,
        "archiveEntry",
        "the inherited source resolves to the same Full row"
    );
    assert_eq!(
        checks::row(&wire, ancestor)?
            .get("path")
            .and_then(|path| path.get("value"))
            .ok_or("retained original source row")?,
        "outer.pak",
        "the root upgrade uses actual root metadata"
    );
    let full_bytes = serde_json::to_vec(&sources.freeze()?)?;
    let limited = support::source(sources, root, SourceDetails::CaptureLimited)?;
    assert_eq!(
        limited.source_ref(),
        ancestor.source_ref(),
        "limited root capture cannot rebind the prefix"
    );
    assert_eq!(
        serde_json::to_vec(&sources.freeze()?)?,
        full_bytes,
        "limited root capture cannot downgrade or duplicate the Full row"
    );
    support::assert_closed(&sources.freeze()?)?;
    Ok(())
}

fn selected_legacy_root(
    sources: &SourceUses,
    root: &Archive,
    deeper: &Archive,
    deep: &SourceScope,
    ancestor: &SourceScope,
    original_use: &SourceUse,
) -> TestResult {
    let full_bytes = serde_json::to_vec(&sources.freeze()?)?;
    let limited = support::source(sources, root, SourceDetails::CaptureLimited)?;
    assert_eq!(
        limited.source_ref(),
        ancestor.source_ref(),
        "explicit View adopts the actual existing Full root reference"
    );
    assert_eq!(
        serde_json::to_vec(&sources.freeze()?)?,
        full_bytes,
        "adopting an existing Full root changes no inventory byte"
    );
    let repeated = checks::full_source(sources, deeper)?;
    assert_ne!(
        repeated.source_ref(),
        deep.source_ref(),
        "legacy unknown leaf observations keep fresh nonces until their own View is explicitly selected"
    );
    assert_eq!(
        repeated
            .ancestor(0)?
            .ok_or("repeated root ancestor")?
            .source_ref(),
        ancestor.source_ref(),
        "a new legacy leaf cannot duplicate its proved root prefix"
    );
    assert_eq!(
        support::used(ancestor)?.reference(),
        original_use.reference(),
        "previous Full source/use refs are never rebound"
    );
    let wire = serde_json::to_value(sources.freeze()?)?;
    assert_eq!(
        checks::physical_rows(&wire)?.len(),
        5,
        "two legacy unknown leaves keep separate unknown rows but share one proved root"
    );
    support::assert_closed(&sources.freeze()?)?;
    Ok(())
}
