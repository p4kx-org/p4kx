// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Source-row lookup and retained legacy sibling references.

use p4kx_archive::{Entry, EntryOrigin, ObservedEntry};
use p4kx_extraction::{Archive, InputScope, SourceDetails, SourceInput, SourceScope, SourceUses};
use serde_json::Value;

use crate::source_minimal_proof as support;
use crate::source_minimal_proof::TestResult;

pub(super) fn full_source(sources: &SourceUses, archive: &Archive) -> TestResult<SourceScope> {
    let origin = archive
        .entry_origin(support::first(archive)?)
        .ok_or("actual full source")?;
    Ok(sources.observe(SourceInput::Archive(&origin), InputScope::Payload)?)
}

pub(super) fn physical_rows(wire: &Value) -> TestResult<&[Value]> {
    wire.get("sources")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "physical source rows".into())
}

pub(super) fn row<'wire>(wire: &'wire Value, scope: &SourceScope) -> TestResult<&'wire Value> {
    physical_rows(wire)?
        .iter()
        .find(|row| {
            row.get("id")
                .is_some_and(|id| id == scope.source_ref().as_str())
        })
        .ok_or_else(|| "retained original source row".into())
}

pub(super) fn known_first_observation(
    sources: &SourceUses,
    separate: &SourceScope,
    ancestor: &SourceScope,
    observed: &ObservedEntry<'_>,
    root: &Archive,
    known: &Entry,
) -> TestResult {
    assert_ne!(
        separate.source_ref(),
        ancestor.source_ref(),
        "a cache miss cannot adopt an unrelated legacy prefix"
    );
    assert!(
        separate.matches_entry(observed)?,
        "the new source keeps its own exact admitted original Entry"
    );
    assert!(
        !separate.matches_entry(
            &root
                .observed_entry(support::first(root)?)
                .ok_or("actual containing Entry")?
        )?,
        "equal root owner is insufficient without equal original coordinates"
    );
    let full = sources.observe(
        SourceInput::Archive(&root.entry_origin(known).ok_or("actual full origin")?),
        InputScope::Payload,
    )?;
    assert_eq!(
        separate.source_ref(),
        full.source_ref(),
        "the new known source upgrades through its canonical coordinate"
    );
    let wire = serde_json::to_value(sources.freeze()?)?;
    assert_eq!(
        physical_rows(&wire)?.len(),
        4,
        "a different known Entry adds exactly one physical source"
    );
    assert_eq!(
        row(&wire, separate)?
            .get("path")
            .and_then(|path| path.get("value"))
            .ok_or("retained original source row")?,
        "different.bin",
        "the unrelated source's actual details remain distinct"
    );
    support::assert_closed(&sources.freeze()?)?;
    Ok(())
}

pub(super) fn selected_sibling_refs(
    sources: &SourceUses,
    view: &Archive,
    first_full: &SourceScope,
    second_full: &SourceScope,
    second_duplicate: &SourceScope,
    second_origin: &EntryOrigin,
    second: &Entry,
) -> TestResult {
    let original_use = support::used(second_full)?;
    let duplicate_use = support::used(second_duplicate)?;
    let before = serde_json::to_vec(&sources.freeze()?)?;
    let selected = support::source(sources, view, SourceDetails::CaptureLimited)?;
    assert_eq!(
        selected.source_ref(),
        first_full.source_ref(),
        "selecting the original view adopts the first sibling's existing Legacy row"
    );
    let second_view = view
        .observed_entry(second)
        .ok_or("actual second sibling view")?;
    let adopted = sources.observe_with_details(
        SourceInput::ArchiveView(second_view),
        InputScope::Payload,
        SourceDetails::CaptureLimited,
    )?;
    assert_eq!(
        adopted.source_ref(),
        second_full.source_ref(),
        "the sibling keeps its own first registered Legacy row although the selected view carries the first sibling's different nonce"
    );
    assert_eq!(
        support::used(&adopted)?.reference(),
        original_use.reference(),
        "explicit View reuse preserves the sibling's existing use reference"
    );
    let repeated = sources.observe(SourceInput::Archive(second_origin), InputScope::Payload)?;
    assert_eq!(
        repeated.source_ref(),
        second_full.source_ref(),
        "Default Full reuses the already selected original view even when its nonce is Legacy"
    );
    assert_eq!(
        serde_json::to_vec(&sources.freeze()?)?,
        before,
        "selection adds no source or use row, cannot downgrade Full and retains all previously issued duplicate refs"
    );
    let inventory = sources.freeze()?;
    assert!(
        inventory
            .uses()
            .iter()
            .any(|used| used.id() == duplicate_use.reference()
                && used.source() == second_duplicate.source_ref()),
        "the unselected historical duplicate's source/use relation remains retained"
    );
    support::assert_closed(&inventory)?;
    Ok(())
}
