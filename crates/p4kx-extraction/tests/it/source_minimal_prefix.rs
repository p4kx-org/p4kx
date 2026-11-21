// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original-prefix identity across independently observed container depths.

use p4kx_extraction::{InputScope, SourceDetails, SourceInput, SourceUses};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::source_minimal_proof as support;
use crate::source_minimal_proof::TestResult;

#[test]
fn shared_unknown_prefix_observed_before_descendant_upgrades_one_ancestor() -> TestResult {
    cases::shared_unknown_prefix_upgrades_one_original_ancestor(false)
}

#[test]
fn shared_unknown_prefix_observed_after_descendant_upgrades_one_ancestor() -> TestResult {
    cases::shared_unknown_prefix_upgrades_one_original_ancestor(true)
}

#[test]
fn known_prefix_observed_before_unknown_descendant_upgrades_one_root() -> TestResult {
    cases::known_prefix_before_an_unknown_view_upgrades_one_root(false)
}

#[test]
fn known_prefix_observed_after_unknown_descendant_upgrades_one_root() -> TestResult {
    cases::known_prefix_before_an_unknown_view_upgrades_one_root(true)
}

#[test]
fn legacy_full_descendant_before_separate_root_keeps_existing_refs() -> TestResult {
    cases::legacy_full_descendants_and_separate_roots_keep_existing_refs(false)
}

#[test]
fn legacy_full_descendant_after_separate_root_keeps_existing_refs() -> TestResult {
    cases::legacy_full_descendants_and_separate_roots_keep_existing_refs(true)
}

#[test]
fn a_known_first_observation_after_a_legacy_prefix_miss_keeps_its_own_original_entry() -> TestResult
{
    let leaf = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.bin", b"same"))
        .build();
    let outer_bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("inner.pak", leaf))
        .build();
    let (_directory, root) = support::open(
        &ArchiveBuilder::new()
            .entry(TestEntry::stored("outer.pak", outer_bytes))
            .entry(TestEntry::stored("different.bin", b"different")),
    )?;
    let outer = root.open_container(support::first(&root)?)?;
    let detached = support::first(&outer)?.clone();
    let deeper = outer.open_container(&detached)?;
    let sources = SourceUses::new();
    let legacy = checks::full_source(&sources, &deeper)?;
    let known = root
        .entries()
        .get(1)
        .ok_or("different admitted root Entry")?;
    let observed = root.observed_entry(known).ok_or("actual different view")?;
    let separate = sources.observe_with_details(
        SourceInput::ArchiveView(observed),
        InputScope::Payload,
        SourceDetails::CaptureLimited,
    )?;
    let ancestor = legacy.ancestor(0)?.ok_or("original legacy root prefix")?;
    checks::known_first_observation(&sources, &separate, &ancestor, &observed, &root, known)?;
    Ok(())
}

#[test]
fn an_explicit_unknown_view_reuses_each_siblings_original_legacy_row() -> TestResult {
    let inner = ArchiveBuilder::new()
        .entry(TestEntry::stored("first.bin", b"one"))
        .entry(TestEntry::stored("second.bin", b"two"))
        .build();
    let (_directory, root) =
        support::open(&ArchiveBuilder::new().entry(TestEntry::stored("room.pak", inner)))?;
    let detached = support::first(&root)?.clone();
    let view = root.open_container(&detached)?;
    let first = support::first(&view)?;
    let second = view
        .entries()
        .get(1)
        .ok_or("second original sibling Entry")?;
    let first_origin = view
        .entry_origin(first)
        .ok_or("first original full origin")?;
    let second_origin = view
        .entry_origin(second)
        .ok_or("second original full origin")?;
    let sources = SourceUses::new();
    let first_full = sources.observe(SourceInput::Archive(&first_origin), InputScope::Payload)?;
    let second_full = sources.observe(SourceInput::Archive(&second_origin), InputScope::Payload)?;
    let second_duplicate =
        sources.observe(SourceInput::Archive(&second_origin), InputScope::Payload)?;
    assert_ne!(
        second_full.source_ref(),
        second_duplicate.source_ref(),
        "before explicit View selection the unknown legacy leaf still receives fresh identities"
    );
    checks::selected_sibling_refs(
        &sources,
        &view,
        &first_full,
        &second_full,
        &second_duplicate,
        &second_origin,
        second,
    )?;
    Ok(())
}

#[path = "source_minimal_prefix_cases.rs"]
mod cases;

#[path = "source_minimal_prefix_checks.rs"]
mod checks;
