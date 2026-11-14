// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataForgeError, TagBuildEvent, TagTree};

use crate::tag_metadata_source as source;

use source::TestResult;

#[derive(Default)]
struct Trace {
    metadata: Vec<(u32, DataForgeError)>,
    reads: Vec<u32>,
    paths: Vec<u32>,
    cancelled: usize,
}

impl Trace {
    fn event(
        &mut self,
        forge: &DataForge<'_>,
        event: &TagBuildEvent<'_, '_>,
    ) -> Result<(), String> {
        match event {
            TagBuildEvent::MetadataFailed { record, error } => {
                if !record.is_from(forge) {
                    return Err("foreign metadata source".to_owned());
                }
                self.metadata.push((record.index(), (*error).clone()));
            }
            TagBuildEvent::CanonicalRead { record, outcome } => {
                let tree = outcome.map_err(ToString::to_string)?;
                if !record.is_from(forge) || !tree.is_from(record) {
                    return Err("foreign canonical source".to_owned());
                }
                self.reads.push(record.index());
            }
            TagBuildEvent::PathChosen { record, .. } => self.paths.push(record.index()),
            TagBuildEvent::Cancelled => self.cancelled += 1,
            _ => {}
        }
        Ok(())
    }
}

#[test]
fn unreadable_family_selector_cannot_produce_a_clean_empty_taxonomy() -> TestResult {
    let source = source::source(false, true)?;
    let forge = DataForge::open(&source.bytes)?;
    let original = source::unknown_original(&forge, &source)?;
    let mut trace = Trace::default();
    let tree =
        TagTree::build_observed(&forge, &|| false, &mut |event| trace.event(&forge, &event))?
            .ok_or("the source read was not cancelled")?;
    assert_eq!(
        trace.metadata,
        [(source.other, original)],
        "the independent actual selector error is retained"
    );
    assert!(
        trace.reads.is_empty() && trace.paths.is_empty(),
        "unknown membership does not invent a Tag or a payload read"
    );
    assert_eq!(
        tree.issues().unreadable(),
        0,
        "no known Tag payload was read and failed"
    );
    assert!(
        tree.is_empty(),
        "no tag can be selected through the unreadable type"
    );
    assert!(
        !tree.issues().is_clean(),
        "unknown source membership cannot prove the taxonomy is clean and absent"
    );
    Ok(())
}

#[test]
fn unreadable_membership_also_marks_an_otherwise_readable_listed_tree_incomplete() -> TestResult {
    let source = source::source(true, true)?;
    let forge = DataForge::open(&source.bytes)?;
    let original = source::unknown_original(&forge, &source)?;
    let mut trace = Trace::default();
    let tree =
        TagTree::build_observed(&forge, &|| false, &mut |event| trace.event(&forge, &event))?
            .ok_or("the actual source pass was not cancelled")?;
    assert_eq!(
        trace.metadata,
        [(source.other, original)],
        "the separate original unclassified row"
    );
    assert_eq!(
        trace.reads,
        [0, 1],
        "only the genuine Tag and Database canonical reads occurred"
    );
    assert_eq!(
        trace.paths,
        [0],
        "the actual readable listed Tag is still selected"
    );
    assert_eq!(
        tree.len(),
        1,
        "unknown membership does not erase the readable subtree"
    );
    assert_eq!(
        tree.iter().next().ok_or("the actual chosen Tag")?.path(),
        "Root",
        "the actual source name survives"
    );
    assert_eq!(
        (tree.issues().unreadable(), tree.issues().unlisted()),
        (0, 0),
        "unknown is neither failed Tag payload nor an unlisted Tag"
    );
    assert!(
        !tree.issues().is_clean(),
        "the known subtree cannot establish complete family selection"
    );
    Ok(())
}

#[test]
fn a_genuinely_known_unrelated_record_allows_clean_absence() -> TestResult {
    let source = source::source(false, false)?;
    let forge = DataForge::open(&source.bytes)?;
    source::other_original(&forge, &source)?;
    let mut trace = Trace::default();
    let tree =
        TagTree::build_observed(&forge, &|| false, &mut |event| trace.event(&forge, &event))?
            .ok_or("an uncancelled source pass")?;
    assert!(
        tree.is_empty() && tree.issues().is_clean(),
        "a readable non-Tag selector establishes actual absence"
    );
    assert!(
        trace.metadata.is_empty() && trace.reads.is_empty() && trace.paths.is_empty(),
        "unrelated sources do not become Tag reads"
    );
    Ok(())
}

#[test]
fn known_clean_taxonomy_stays_clean() -> TestResult {
    let source = source::source(true, false)?;
    let forge = DataForge::open(&source.bytes)?;
    source::other_original(&forge, &source)?;
    let mut trace = Trace::default();
    let tree =
        TagTree::build_observed(&forge, &|| false, &mut |event| trace.event(&forge, &event))?
            .ok_or("an uncancelled source pass")?;
    assert_eq!(tree.len(), 1, "the actual listed Tag is present");
    assert!(
        tree.issues().is_clean(),
        "known unrelated sources do not taint the clean taxonomy"
    );
    assert_eq!(trace.reads, [0, 1]);
    assert!(
        trace.metadata.is_empty(),
        "readable original selectors have no metadata failure"
    );
    Ok(())
}

#[test]
fn cancellation_does_not_invent_failed_family_observations() -> TestResult {
    let source = source::source(false, true)?;
    let forge = DataForge::open(&source.bytes)?;
    source::unknown_original(&forge, &source)?;
    let mut trace = Trace::default();
    let tree = TagTree::build_observed(&forge, &|| true, &mut |event| trace.event(&forge, &event))?;
    assert!(
        tree.is_none(),
        "the existing cancellation result remains distinct from a partial tree"
    );
    assert_eq!(trace.cancelled, 1);
    assert!(
        trace.metadata.is_empty() && trace.reads.is_empty(),
        "an unvisited selector has no fabricated event"
    );
    Ok(())
}

#[test]
fn rejecting_metadata_observation_keeps_the_original_observer_error() -> TestResult {
    let source = source::source(false, true)?;
    let forge = DataForge::open(&source.bytes)?;
    source::unknown_original(&forge, &source)?;
    let mut seen = 0;
    let result = TagTree::build_observed(&forge, &|| false, &mut |event| {
        if matches!(event, TagBuildEvent::MetadataFailed { .. }) {
            seen += 1;
            return Err("caller rejected metadata");
        }
        Ok(())
    });
    assert_eq!(
        result.err(),
        Some("caller rejected metadata"),
        "the existing observer error is not replaced by a tree"
    );
    assert_eq!(
        seen, 1,
        "the actual selector error is observed exactly once"
    );
    Ok(())
}
