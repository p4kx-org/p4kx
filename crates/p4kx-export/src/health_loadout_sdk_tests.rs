// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{
    LoadoutFileError, LoadoutFiles, LoadoutPath, LoadoutReadEvent, LoadoutReadObserver,
};
use p4kx_extraction::{CancelToken, SourceEvidenceError, SourceLocation, SourceUseRole};

use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::ship_pool_test_input;
use crate::catalog_read::original::AuthoredEntryPath;
use crate::{CatalogError, ExportAccountingError, HealthText};

#[path = "health_loadout_sdk_fixture.rs"]
mod fixture;
use fixture::{Probe, TestResult};

#[test]
fn actual_include_parent_is_projected_before_completed_or_failed_child_edges() -> TestResult {
    for child in [Some(fixture::CHILD_XML), Some(b"<broken>".as_slice()), None] {
        ship_pool_test_input::with_input(
            &fixture::database(),
            fixture::entries(child),
            |archive, files, forge, input| {
                fixture::original(archive, forge, child)?;
                let probe = Arc::new(Probe::new(input, None));
                let capture = &probe.capture;
                fixture::resolve(forge, files, Arc::clone(&probe), true)?;
                capture.finish()?;
                let seen = probe.seen.lock().map_err(|_| "SDK source observations")?;
                assert_eq!(seen.reads.len(), 2, "only the two actual file operations");
                assert_eq!(seen.edges.len(), 1, "one genuine direct Include edge");
                assert_eq!(seen.reused, 1, "only the cached root is replayed");
                let edge = seen.edges.first().ok_or("actual Include edge")?;
                assert_eq!(
                    edge.parent.projection_outcome(),
                    Some(Ok(())),
                    "actual parent parse precedes its Include"
                );
                match child {
                    None => assert!(
                        matches!(edge.child.read_outcome(), Err(LoadoutFileError::NotFound)),
                        "genuine missing read"
                    ),
                    Some(value) if value == fixture::CHILD_XML => assert_eq!(
                        edge.child.projection_outcome(),
                        Some(Ok(())),
                        "genuine successful child parse"
                    ),
                    Some(_) => assert!(
                        edge.child
                            .projection_outcome()
                            .is_some_and(|outcome| outcome.is_err()),
                        "genuine failed child parse"
                    ),
                }
                drop(seen);
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn actual_archive_and_unbound_xml_cells_retain_distinct_source_knowledge() -> TestResult {
    ship_pool_test_input::with_input(
        &fixture::database(),
        fixture::entries(Some(fixture::CHILD_XML)),
        |archive, files, forge, input| {
            fixture::original(archive, forge, Some(fixture::CHILD_XML))?;
            for (provider, complete) in [
                (files as &dyn LoadoutFiles, true),
                (&fixture::Generic, false),
            ] {
                let probe = Arc::new(Probe::new(input, None));
                let capture = &probe.capture;
                fixture::resolve(forge, provider, Arc::clone(&probe), false)?;
                capture.finish()?;
                let seen = probe.seen.lock().map_err(|_| "SDK source observations")?;
                let (node, recipe) = seen.paths.first().ok_or("actual same-parse Include site")?;
                check_cell(recipe, *node, input, complete)?;
                assert_eq!(seen.paths.len(), 1, "exactly one authored XML path getter");
                assert_eq!(seen.reads.len(), 2, "both genuine reads occurred");
                assert_eq!(
                    seen.reads
                        .iter()
                        .all(|read| read.source().archive_input_origin().is_some()),
                    complete,
                    "actual provider admissions"
                );
            }
            Ok(())
        },
    )
}

fn check_cell(
    recipe: &AuthoredEntryPath,
    node: u32,
    input: &OriginalCatalogInput<'_, '_>,
    complete: bool,
) -> TestResult {
    assert_eq!(
        node,
        fixture::INCLUDE_NODE,
        "the independently read original XML node"
    );
    let fact = recipe.retain()?;
    let (uses, known) = fact.contributions();
    assert_eq!(known, complete, "actual physical source knowledge");
    assert_eq!(uses.len(), 2, "exact XML payload and decoder contributions");
    for role in [SourceUseRole::Payload, SourceUseRole::Conversion] {
        let contribution = uses
            .iter()
            .find(|candidate| candidate.role() == role)
            .ok_or("actual contribution role")?;
        assert_eq!(
            contribution.description().location(),
            &SourceLocation::Subresource {
                namespace: "xml".to_owned(),
                indices: vec![u64::from(node)],
                field: "@loadout".to_owned(),
            },
            "the actual same-parse node and winning attribute"
        );
    }
    let cell = fact.into_cell(&input.origins().uses);
    assert_eq!(
        cell.value().map(HealthText::as_str),
        Some(fixture::CHILD),
        "exact authored Include text"
    );
    assert_eq!(
        cell.origin().source_set_complete(),
        complete,
        "own immediate cell completeness"
    );
    assert_eq!(
        cell.origin().contains_encrypted_source(),
        complete.then_some(false),
        "genuine Unbound is never false-plain"
    );
    assert_eq!(
        cell.origin().source_uses_refs().len(),
        2,
        "both exact source-use references"
    );
    Ok(())
}

#[test]
fn genuine_equal_archive_descriptors_cannot_supply_another_inputs_file_reads() -> TestResult {
    ship_pool_test_input::with_input(
        &fixture::database(),
        fixture::entries(Some(fixture::CHILD_XML)),
        |archive, files, forge, input| {
            fixture::original(archive, forge, Some(fixture::CHILD_XML))?;
            ship_pool_test_input::with_input(
                &fixture::database(),
                fixture::entries(Some(fixture::CHILD_XML)),
                |foreign_archive, foreign_files, foreign_forge, _| {
                    fixture::original(foreign_archive, foreign_forge, Some(fixture::CHILD_XML))?;
                    assert_eq!(
                        files.identity(fixture::ROOT),
                        foreign_files.identity(fixture::ROOT),
                        "same genuine path/index descriptions"
                    );
                    let cancel = CancelToken::new();
                    let probe = Arc::new(Probe::new(input, Some(&cancel)));
                    let capture = &probe.capture;
                    assert!(
                        fixture::resolve(forge, foreign_files, Arc::clone(&probe), false).is_err(),
                        "foreign actual byte operation stops the resolver"
                    );
                    cancel.cancel();
                    assert!(
                        matches!(
                            capture.finish(),
                            Err(CatalogError::SourceEvidence(
                                SourceEvidenceError::ForeignEvidence
                            ))
                        ),
                        "the first genuine foreign-input cause survives later cancellation"
                    );
                    let seen = probe.seen.lock().map_err(|_| "SDK source observations")?;
                    assert_eq!(
                        seen.reads.len(),
                        1,
                        "only the foreign root read was attempted"
                    );
                    assert!(
                        seen.paths.is_empty() && seen.edges.is_empty(),
                        "no parse or dependency after the rejected admission"
                    );
                    drop(seen);
                    Ok(())
                },
            )
        },
    )
}

#[test]
fn actual_dependency_rejects_a_different_real_parent_and_preserves_first_error() -> TestResult {
    ship_pool_test_input::with_input(
        &fixture::database(),
        fixture::entries(Some(fixture::CHILD_XML)),
        |archive, files, forge, input| {
            fixture::original(archive, forge, Some(fixture::CHILD_XML))?;
            let cancel = CancelToken::new();
            let probe = Arc::new(Probe::new(input, Some(&cancel)));
            let capture = &probe.capture;
            fixture::resolve(forge, files, Arc::clone(&probe), false)?;
            capture.finish()?;
            let edge = probe
                .seen
                .lock()
                .map_err(|_| "SDK source observations")?
                .edges
                .first()
                .cloned()
                .ok_or("actual direct edge")?;
            assert!(
                !edge.parent.is_same_operation(&edge.child),
                "two genuine different original operations"
            );
            assert_eq!(
                edge.child.projection_outcome(),
                Some(Ok(())),
                "the substituted parent also genuinely parsed"
            );
            let before = input.origins().uses.freeze()?;
            assert!(
                capture
                    .observe(LoadoutReadEvent::FileDependency {
                        parent: &edge.child,
                        observation: &edge.child,
                        request: &edge.request,
                        dependency: &edge.dependency
                    })
                    .is_break(),
                "only the original owning parse admits the Include selection"
            );
            cancel.cancel();
            assert!(
                matches!(
                    capture.finish(),
                    Err(CatalogError::Accounting(
                        ExportAccountingError::ConflictingSourceDatum
                    ))
                ),
                "the original parent rejection precedes cancellation"
            );
            assert_eq!(
                before,
                input.origins().uses.freeze()?,
                "rejected edge contributes no payload"
            );
            Ok(())
        },
    )
}

#[test]
fn a_genuine_whole_request_clone_is_valid_but_its_copied_text_is_not() -> TestResult {
    ship_pool_test_input::with_input(
        &fixture::database(),
        fixture::entries(Some(fixture::CHILD_XML)),
        |archive, files, forge, input| {
            fixture::original(archive, forge, Some(fixture::CHILD_XML))?;
            let probe = Arc::new(Probe::new(input, None));
            let capture = &probe.capture;
            fixture::resolve(forge, files, Arc::clone(&probe), false)?;
            capture.finish()?;
            let original = probe
                .seen
                .lock()
                .map_err(|_| "SDK source observations")?
                .first
                .clone()
                .ok_or("actual original read")?;
            assert_eq!(
                original.request.value(),
                fixture::ROOT,
                "the actual original candidate"
            );
            assert!(
                original.request.selection().is_some(),
                "actual Getter receipt survives its whole clone"
            );
            assert!(
                capture
                    .observe(LoadoutReadEvent::FileReused {
                        observation: &original.original,
                        request: &original.request,
                        caller: &original.caller
                    })
                    .is_continue(),
                "the genuine observed request clone remains valid"
            );
            capture.finish()?;
            reject_copied_request(capture, input, &original)
        },
    )
}

fn reject_copied_request(
    capture: &crate::loadout::ledger::Capture<'_>,
    input: &OriginalCatalogInput<'_, '_>,
    original: &fixture::ReadContext,
) -> TestResult {
    let copied = LoadoutPath::unbound(original.request.value());
    assert_eq!(
        copied.value(),
        original.request.value(),
        "exact copied descriptor text"
    );
    assert!(
        copied.selection().is_none(),
        "copied text has no source selection"
    );
    let before = input.origins().uses.freeze()?;
    assert!(
        capture
            .observe(LoadoutReadEvent::FileReused {
                observation: &original.original,
                request: &copied,
                caller: &original.caller
            })
            .is_break(),
        "matching text cannot replace the original candidate receipt"
    );
    assert!(
        matches!(
            capture.finish(),
            Err(CatalogError::Accounting(
                ExportAccountingError::UnboundHealthSubject
            ))
        ),
        "the missing source selection remains explicit"
    );
    assert_eq!(
        before,
        input.origins().uses.freeze()?,
        "no new contributions after descriptor rejection"
    );
    Ok(())
}

#[test]
fn cancellation_before_the_actual_getters_allows_no_file_or_xml_observations() -> TestResult {
    ship_pool_test_input::with_input(
        &fixture::database(),
        fixture::entries(Some(fixture::CHILD_XML)),
        |archive, files, forge, input| {
            fixture::original(archive, forge, Some(fixture::CHILD_XML))?;
            let cancel = CancelToken::new();
            let probe = Arc::new(Probe::new(input, Some(&cancel)));
            let capture = &probe.capture;
            let before = input.origins().uses.freeze()?;
            cancel.cancel();
            assert!(
                fixture::resolve(forge, files, Arc::clone(&probe), false).is_err(),
                "the actual resolver stops before file reads"
            );
            assert!(
                matches!(capture.finish(), Err(CatalogError::Cancelled)),
                "the original cancellation reaches the caller"
            );
            let seen = probe.seen.lock().map_err(|_| "SDK source observations")?;
            assert!(
                seen.reads.is_empty() && seen.paths.is_empty() && seen.edges.is_empty(),
                "no original file or XML operation after cancellation"
            );
            drop(seen);
            assert_eq!(
                before,
                input.origins().uses.freeze()?,
                "cancellation admits no new source uses"
            );
            Ok(())
        },
    )
}

#[path = "health_loadout_intrinsic_scope_tests.rs"]
mod intrinsic_scope;
