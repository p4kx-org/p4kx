// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::Ordering;

use p4kx_assembly::{Entities, LoadoutResolver};
use p4kx_dataforge::DataForge;

use crate::loadout_cache_shared_support as fixture;
use crate::loadout_cache_shared_support::{Counts, Files, TestResult};

#[test]
fn a_complete_shared_dag_does_not_repeat_transitive_receipts_per_include() -> TestResult {
    let database = fixture::database();
    let forge = DataForge::open(&database)?;
    let files = fixture::files();
    let counts = Arc::new(Counts::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), counts.clone())
            .with_files(&files);
    let root = forge.record(0).ok_or("the synthetic Entity is absent")?;
    let node = resolver.resolve(&root)?;
    assert!(
        node.issues().is_empty(),
        "a shallow valid DAG requires no omitted input"
    );
    verify_initial_counts(&files, &counts);
    resolver.resolve(&root)?;
    assert_eq!(
        counts.reused.load(Ordering::Relaxed),
        1,
        "global reuse adds one current-caller root link"
    );
    assert_eq!(
        counts.dependencies.load(Ordering::Relaxed),
        21,
        "cache reuse does not repeat original edges"
    );
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        7,
        "the root link performs no provider read"
    );
    Ok(())
}

#[test]
fn the_complete_shared_dag_keeps_the_unobserved_read_count() -> TestResult {
    let database = fixture::database();
    let forge = DataForge::open(&database)?;
    let files = fixture::files();
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let root = forge.record(0).ok_or("the synthetic Entity is absent")?;
    let node = resolver.resolve(&root)?;
    assert!(
        node.issues().is_empty(),
        "the ordinary valid DAG remains fully readable"
    );
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        7,
        "observation must not add source reads"
    );
    Ok(())
}

fn verify_initial_counts(files: &Files, counts: &Counts) {
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        7,
        "every original file is read once"
    );
    assert_eq!(
        counts.reads.load(Ordering::Relaxed),
        7,
        "only genuine file reads are observed"
    );
    assert_eq!(
        counts.projections.load(Ordering::Relaxed),
        7,
        "only genuine parses are observed"
    );
    assert_eq!(
        counts.incomplete.load(Ordering::Relaxed),
        0,
        "the oracle reaches no input limit"
    );
    assert_eq!(
        counts.reused.load(Ordering::Relaxed),
        0,
        "direct Include edges must reference original receipts without transitive per-caller replay"
    );
    assert_eq!(
        counts.dependencies.load(Ordering::Relaxed),
        21,
        "six shared documents and their root have exactly twenty-one direct Includes"
    );
    assert_eq!(
        counts.refusals.load(Ordering::Relaxed),
        0,
        "the complete DAG has no refused Include"
    );
}
