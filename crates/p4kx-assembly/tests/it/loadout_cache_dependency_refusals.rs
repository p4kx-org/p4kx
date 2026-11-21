// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{Attachment, Entities, LoadoutError, LoadoutResolver, LoadoutXmlClassUse};
use p4kx_dataforge::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestLoadout};

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
            Err(LoadoutError::ObservationStopped)
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
fn an_explicit_include_budget_refuses_the_xml_result_before_the_next_read() -> TestResult {
    const INCLUDES: usize = 257;
    let mut documents = HashMap::new();
    let budget = INCLUDES - 1;
    documents.insert(
        fixture::ROOT.to_owned(),
        fixture::document((0..INCLUDES).map(fixture::path)),
    );
    for index in 0..INCLUDES {
        documents.insert(fixture::path(index), fixture::document([]));
    }
    documents.insert(
        fixture::path(0),
        br#"<Loadout><Items><Item portName="admitted"/></Items></Loadout>"#.to_vec(),
    );
    verify_refusal(
        &Files {
            documents,
            reads: AtomicUsize::new(0),
        },
        Some(budget),
        budget + 1,
        &fixture::path(budget),
    )
}

#[test]
fn a_nested_budget_refusal_records_only_the_unread_dependency() -> TestResult {
    let branch = fixture::path(0);
    let completed = fixture::path(1);
    let refused = fixture::path(2);
    let documents = HashMap::from([
        (
            fixture::ROOT.to_owned(),
            fixture::document([branch.clone()]),
        ),
        (
            branch,
            fixture::document([completed.clone(), refused.clone()]),
        ),
        (completed, fixture::document([])),
    ]);
    verify_refusal(
        &Files {
            documents,
            reads: AtomicUsize::new(0),
        },
        Some(2),
        3,
        &refused,
    )
}

const DEEP_DOCUMENTS: usize = 257;

#[test]
fn deep_includes_preserve_authored_edges_and_the_leaf_route_on_a_small_stack() -> TestResult {
    let worker = std::thread::Builder::new()
        .stack_size(128 << 10)
        .spawn(|| prove_deep_includes().map_err(|error| error.to_string()))?;
    worker
        .join()
        .map_err(|_| "deep Include worker panicked")??;
    Ok(())
}

fn prove_deep_includes() -> TestResult {
    let (files, paths) = deep_files();
    let builder = p4kx_testkit::entity_schema(DataForgeBuilder::new());
    let database = builder
        .record(
            TestEntity::new(1, "Root", "synthetic/entity.xml")
                .loadout(TestLoadout::Xml(fixture::ROOT.to_owned()))
                .record(),
        )
        .record(TestEntity::new(2, "Leaf", "synthetic/leaf.xml").record())
        .build();
    let forge = DataForge::open(&database)?;
    let probe = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone())
            .with_files(&files);
    let root = forge.record(0).ok_or("the synthetic Entity is absent")?;
    let node = resolver.resolve(&root)?;
    assert!(node.issues().is_empty(), "every deep document was resolved");
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        DEEP_DOCUMENTS,
        "each document is read once despite repeated Includes"
    );
    let [port] = node.ports() else {
        return Err("only the leaf contributed one port".into());
    };
    let Attachment::Entity(leaf) = port.attachment() else {
        return Err("the deep port did not attach the selected Entity".into());
    };
    let usage = leaf
        .attachment_target()
        .and_then(p4kx_assembly::LoadoutTargetGuid::class_xml_use)
        .ok_or("the selected leaf origin is absent")?;
    let actual: Vec<_> = usage.includes().map(|(_, _, path)| path.value()).collect();
    assert_eq!(
        actual,
        paths.iter().skip(1).map(String::as_str).collect::<Vec<_>>(),
        "the full selected original Include path"
    );

    verify_deep_receipts(&probe, usage, &paths)?;
    drop(resolver);
    drop(node);
    Ok(())
}

fn verify_deep_receipts(probe: &Probe, usage: &LoadoutXmlClassUse, paths: &[String]) -> TestResult {
    let observed = probe
        .0
        .lock()
        .map_err(|_| "the deep receipt lock is poisoned")?;
    let reads = observed
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Read)
        .map(|event| event.request.value());
    assert!(
        reads.eq(paths.iter().map(String::as_str)),
        "provider read order"
    );
    let dependencies: Vec<_> = observed
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Dependency)
        .collect();
    assert_eq!(
        dependencies.len(),
        DEEP_DOCUMENTS - 1,
        "every distinct Include produced its edge"
    );
    for (dependency, ((parent, _, requested), expected)) in dependencies
        .iter()
        .rev()
        .zip(usage.includes().zip(paths.iter().skip(1)))
    {
        assert_eq!(requested.value(), expected, "leaf route is in source order");
        assert!(
            dependency
                .parent
                .as_ref()
                .is_some_and(|seen| seen.is_same_operation(parent)),
            "route and dependency retain the actual original parent read"
        );
        let position = dependency
            .dependency
            .as_ref()
            .and_then(source::caller_path)
            .ok_or("the dependency occurrence is absent")?;
        assert_eq!(
            position.selection(),
            requested.selection(),
            "the route preserves the exact original Include occurrence"
        );
    }
    assert!(
        observed.refusals.is_empty(),
        "the valid chain has no refusal"
    );
    drop(observed);
    Ok(())
}

#[test]
fn stopping_on_the_first_deep_dependency_prevents_later_receipts() -> TestResult {
    let (files, _) = deep_files();
    verify_stop(
        &files,
        Counts {
            stop_on_dependency: true,
            ..Counts::default()
        },
        DEEP_DOCUMENTS,
        1,
        0,
    )
}

fn deep_files() -> (Files, Vec<String>) {
    let paths: Vec<_> = std::iter::once(fixture::ROOT.to_owned())
        .chain((1..DEEP_DOCUMENTS).map(fixture::path))
        .collect();
    let mut documents: HashMap<_, _> = paths
        .iter()
        .zip(paths.iter().skip(1))
        .map(|(path, next)| {
            (
                path.clone(),
                fixture::document([next.clone(), next.to_ascii_uppercase()]),
            )
        })
        .collect();
    if let Some(leaf) = paths.last() {
        documents.insert(
            leaf.clone(),
            br#"<Loadout><Items><Item portName="terminal" itemName="Leaf"/></Items></Loadout>"#
                .to_vec(),
        );
    }
    (
        Files {
            documents,
            reads: AtomicUsize::new(0),
        },
        paths,
    )
}

fn verify_refusal(files: &Files, budget: Option<usize>, reads: usize, skipped: &str) -> TestResult {
    let database = fixture::database();
    let forge = DataForge::open(&database)?;
    let probe = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone())
            .with_files(files)
            .with_xml_include_read_budget(budget);
    let root = forge.record(0).ok_or("the synthetic Entity is absent")?;
    let node = resolver.resolve(&root)?;
    assert_eq!(
        node.issues().len(),
        1,
        "the actual first omitted Include keeps its precise issue"
    );
    if let Some(limit) = budget {
        assert!(
            matches!(node.issues(), [LoadoutError::XmlIncludeReadBudgetExceeded { path, limit: actual }] if path == skipped && *actual == limit),
            "the caller sees the precise budget error"
        );
        assert!(
            node.ports().is_empty(),
            "the refused XML contributes no partial ports"
        );
    }
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        reads,
        "a readless refusal adds no provider read"
    );
    verify_source_refusal(&probe, skipped)
}

fn verify_source_refusal(probe: &Probe, skipped: &str) -> TestResult {
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
