// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{Entities, LoadoutResolver};
use p4kx_dataforge::DataForge;

use crate::loadout_cache_shared_support as fixture;
use crate::loadout_cache_shared_support::{Counts, Files, TestResult};
use crate::loadout_request_authorship_support as source;
use crate::loadout_request_authorship_support::{EventKind, Probe};

#[test]
fn stopping_on_a_direct_edge_does_not_emit_another_operation() -> TestResult {
    verify_stop(
        &fixture::files(),
        Counts {
            stop_on_dependency: true,
            ..Counts::default()
        },
        2,
        1,
        0,
    )
}

#[test]
fn stopping_on_a_readless_refusal_does_not_emit_another_operation() -> TestResult {
    let mut documents = HashMap::new();
    documents.insert(
        fixture::ROOT.to_owned(),
        fixture::document([fixture::ROOT.to_owned()]),
    );
    verify_stop(
        &Files {
            documents,
            reads: AtomicUsize::new(0),
        },
        Counts {
            stop_on_refusal: true,
            ..Counts::default()
        },
        1,
        0,
        1,
    )
}

fn verify_stop(
    files: &Files,
    counts: Counts,
    reads: usize,
    edges: usize,
    refusals: usize,
) -> TestResult {
    let database = fixture::database();
    let forge = DataForge::open(&database)?;
    let counts = Arc::new(counts);
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), counts.clone())
            .with_files(files);
    let root = forge.record(0).ok_or("the synthetic Entity is absent")?;
    assert!(
        matches!(
            resolver.resolve(&root),
            Err(p4kx_assembly::LoadoutError::ObservationStopped)
        ),
        "an edge or refusal observer can stop before subsequent dependency work"
    );
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        reads,
        "no provider read follows observer stop"
    );
    assert_eq!(
        counts.reads.load(Ordering::Relaxed),
        reads,
        "every read callback precedes the stop"
    );
    assert_eq!(
        counts.projections.load(Ordering::Relaxed),
        reads,
        "no projection follows observer stop"
    );
    assert_eq!(
        counts.dependencies.load(Ordering::Relaxed),
        edges,
        "no successful dependency follows stop"
    );
    assert_eq!(
        counts.refusals.load(Ordering::Relaxed),
        refusals,
        "no extra refusal follows stop"
    );
    assert_eq!(
        counts.reused.load(Ordering::Relaxed),
        0,
        "stop must not invent a successful reuse"
    );
    Ok(())
}

#[test]
fn a_before_read_capacity_refusal_has_no_invented_child_operation() -> TestResult {
    let mut documents = HashMap::new();
    let total = p4kx_assembly::MAX_XML_INCLUDES;
    documents.insert(
        fixture::ROOT.to_owned(),
        fixture::document((0..total).map(fixture::path)),
    );
    for index in 0..total {
        documents.insert(fixture::path(index), fixture::document([]));
    }
    verify_refusal(
        &Files {
            documents,
            reads: AtomicUsize::new(0),
        },
        total,
        &fixture::path(total - 1),
    )
}

#[test]
fn a_before_read_depth_refusal_has_no_invented_child_operation() -> TestResult {
    let mut documents = HashMap::new();
    let total = p4kx_assembly::MAX_XML_INCLUDE_DEPTH;
    documents.insert(
        fixture::ROOT.to_owned(),
        fixture::document([fixture::path(0)]),
    );
    for index in 0..total {
        documents.insert(
            fixture::path(index),
            fixture::document([fixture::path(index + 1)]),
        );
    }
    verify_refusal(
        &Files {
            documents,
            reads: AtomicUsize::new(0),
        },
        total,
        &fixture::path(total - 1),
    )
}

fn verify_refusal(files: &Files, reads: usize, skipped: &str) -> TestResult {
    let database = fixture::database();
    let forge = DataForge::open(&database)?;
    let probe = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone())
            .with_files(files);
    let root = forge.record(0).ok_or("the synthetic Entity is absent")?;
    let node = resolver.resolve(&root)?;
    assert_eq!(
        node.issues().len(),
        1,
        "the actual first omitted Include keeps its precise issue"
    );
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        reads,
        "a readless refusal adds no provider read"
    );
    let state = probe
        .0
        .lock()
        .map_err(|_| "the original-source evidence mutex is poisoned")?;
    assert_eq!(
        state.refusals.len(),
        1,
        "the selected readless refusal is explicit"
    );
    let refusal = state
        .refusals
        .first()
        .ok_or("the genuine readless refusal is absent")?;
    let dependency =
        source::caller_path(&refusal.dependency).ok_or("the genuine Include is absent")?;
    assert_eq!(
        dependency.value(),
        skipped,
        "the first skipped Include retains its own datum"
    );
    let site = state
        .xml
        .iter()
        .find(|site| site.value == skipped && site.observation.is_same_operation(&refusal.parent))
        .ok_or("the original refusal site is absent")?;
    assert_eq!(
        dependency.selection(),
        Some(&site.selection),
        "the refusal retains its real selection"
    );
    assert!(
        !state
            .events
            .iter()
            .any(|event| event.kind == EventKind::Read && event.request.value() == skipped),
        "a pre-read refusal must not create a completed child receipt"
    );
    drop(state);
    Ok(())
}
