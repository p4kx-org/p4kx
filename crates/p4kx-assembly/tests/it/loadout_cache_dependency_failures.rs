// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{Entities, LoadoutFileError, LoadoutResolver};
use p4kx_dataforge::DataForge;

use crate::loadout_cache_shared_support as fixture;
use crate::loadout_cache_shared_support::{Files, TestResult};
use crate::loadout_request_authorship_support as source;
use crate::loadout_request_authorship_support::{EventKind, Probe, State};

#[test]
fn failed_children_and_readless_cycle_keep_different_original_outcomes() -> TestResult {
    let missing = "synthetic/complete/missing.xml";
    let broken = "synthetic/complete/broken.xml";
    let mut documents = HashMap::new();
    documents.insert(
        fixture::ROOT.to_owned(),
        fixture::document([
            missing.to_owned(),
            broken.to_owned(),
            fixture::ROOT.to_owned(),
        ]),
    );
    documents.insert(broken.to_owned(), b"<broken>".to_vec());
    let files = Files {
        documents,
        reads: AtomicUsize::new(0),
    };
    let database = fixture::database();
    let forge = DataForge::open(&database)?;
    let probe = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone())
            .with_files(&files);
    let root = forge.record(0).ok_or("the synthetic Entity is absent")?;
    let node = resolver.resolve(&root)?;
    assert_eq!(
        node.issues().len(),
        3,
        "missing, malformed and cyclic Includes keep precise failures"
    );
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        3,
        "a cycle must not invent another provider read"
    );
    let state = probe
        .0
        .lock()
        .map_err(|_| "the original-source evidence mutex is poisoned")?;
    verify_failed_children(&state, missing, broken)?;
    verify_cycle_refusal(&state)?;
    drop(state);
    Ok(())
}

fn verify_failed_children(state: &State, missing: &str, broken: &str) -> TestResult {
    let edges: Vec<_> = state
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Dependency)
        .collect();
    assert_eq!(
        edges.len(),
        2,
        "only real missing and malformed child operations have direct edges"
    );
    let absent = edges
        .iter()
        .find(|edge| edge.request.value() == missing)
        .ok_or("the missing edge is absent")?;
    assert!(
        matches!(
            absent.observation.read_outcome(),
            Err(LoadoutFileError::NotFound)
        ),
        "a missing edge retains its genuine failed read receipt"
    );
    assert!(
        absent.observation.projection_outcome().is_none(),
        "absence must not invent a parse"
    );
    let malformed = edges
        .iter()
        .find(|edge| edge.request.value() == broken)
        .ok_or("the malformed edge is absent")?;
    assert!(
        malformed.observation.read_outcome().is_ok(),
        "the malformed source was really read"
    );
    assert!(
        malformed
            .observation
            .projection_outcome()
            .is_some_and(|outcome| outcome.is_err()),
        "the malformed edge retains its genuine failed projection"
    );
    Ok(())
}

fn verify_cycle_refusal(state: &State) -> TestResult {
    assert_eq!(
        state.refusals.len(),
        1,
        "the active cycle has one explicit readless refusal"
    );
    let refusal = state
        .refusals
        .first()
        .ok_or("the genuine cycle refusal is absent")?;
    assert!(
        matches!(&refusal.error, p4kx_assembly::LoadoutError::XmlLoadout { path, reason }
        if path == fixture::ROOT && reason == "it includes itself"),
        "the actual precise cycle error is retained"
    );
    let site = state
        .xml
        .iter()
        .find(|site| {
            site.value == fixture::ROOT && site.observation.is_same_operation(&refusal.parent)
        })
        .ok_or("the cycle's actual XML site is absent")?;
    assert_eq!(
        source::caller_path(&refusal.dependency).and_then(p4kx_assembly::LoadoutPath::selection),
        Some(&site.selection),
        "a readless refusal retains its genuine Include selection"
    );
    Ok(())
}
