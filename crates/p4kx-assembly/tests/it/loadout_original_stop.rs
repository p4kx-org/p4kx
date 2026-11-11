// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{
    Entities, LoadoutError, LoadoutFileError, LoadoutFiles, LoadoutReadEvent, LoadoutReadObserver,
    LoadoutResolver,
};
use p4kx_dataforge::{DataForge, Record};

use crate::loadout_original_controls_support::{self as support, TestResult};

struct Provider {
    reads: AtomicUsize,
    evidence_failure: bool,
}

impl LoadoutFiles for Provider {
    fn read(&self, _path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        if self.evidence_failure {
            Err(LoadoutFileError::SourceEvidenceUnavailable)
        } else {
            Ok(b"<Loadout/>".to_vec())
        }
    }
}

#[derive(Default)]
struct Probe {
    events: AtomicUsize,
    projections: AtomicUsize,
    source_failures: AtomicUsize,
    stop_at_projection: bool,
}

impl LoadoutReadObserver for Probe {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        self.events.fetch_add(1, Ordering::Relaxed);
        match event {
            LoadoutReadEvent::FileProjection { .. } => {
                self.projections.fetch_add(1, Ordering::Relaxed);
                if self.stop_at_projection {
                    return ControlFlow::Break(());
                }
            }
            LoadoutReadEvent::FileRead { observation, .. }
                if matches!(
                    observation.read_outcome(),
                    Err(LoadoutFileError::SourceEvidenceUnavailable)
                ) =>
            {
                self.source_failures.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}

fn assert_roots_stopped<'forge>(
    resolver: &mut LoadoutResolver<'forge>,
    callers: [Record<'forge>; 2],
) {
    for record in callers {
        assert!(
            matches!(
                resolver.resolve(&record),
                Err(LoadoutError::ObservationStopped)
            ),
            "an already stopped resolver does not start another original Record read"
        );
        assert!(
            matches!(
                resolver.item_facts(&record),
                Err(LoadoutError::ObservationStopped)
            ),
            "the facts API does not resume the stopped original Record reader"
        );
        assert!(
            matches!(
                resolver.wearable(&record),
                Err(LoadoutError::ObservationStopped)
            ),
            "the wearable API does not resume the stopped original Record reader"
        );
    }
    assert!(
        matches!(
            resolver.resolve_file(support::CONFIGURED),
            Err(LoadoutError::ObservationStopped)
        ),
        "an already stopped file cache does not expose a usable ordinary result"
    );
    assert_eq!(
        resolver.serial_reads(),
        0,
        "no suppressed decode is counted as an actual serial read"
    );
}

fn assert_outcomes(files: &Provider, observer: &Probe, callbacks: usize, evidence_failure: bool) {
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        1,
        "stopped cache reuse performs no new I/O"
    );
    assert_eq!(
        observer.events.load(Ordering::Relaxed),
        callbacks,
        "the sticky first stop suppresses subsequent callbacks as well as I/O"
    );
    assert_eq!(
        observer.projections.load(Ordering::Relaxed),
        usize::from(!evidence_failure),
        "a failed original admission never becomes a parse completion"
    );
    assert_eq!(
        observer.source_failures.load(Ordering::Relaxed),
        usize::from(evidence_failure),
        "the exact original typed evidence failure is visible once before the fatal stop"
    );
}

fn verify_stop(evidence_failure: bool) -> TestResult {
    let database = support::database();
    let forge = DataForge::open(&database)?;
    let callers = support::callers(&forge)?;
    let files = Provider {
        reads: AtomicUsize::new(0),
        evidence_failure,
    };
    let observer = Arc::new(Probe {
        stop_at_projection: !evidence_failure,
        ..Probe::default()
    });
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone())
            .with_files(&files);
    assert!(
        matches!(
            resolver.resolve_file(support::CONFIGURED),
            Err(LoadoutError::ObservationStopped)
        ),
        "the actual parse callback or typed evidence failure is fatal at the file-root API"
    );
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        1,
        "one original operation reaches the provider"
    );
    let callbacks = observer.events.load(Ordering::Relaxed);
    assert!(
        callbacks > 0,
        "the genuine operation reaches the source observer before its stop"
    );
    assert_roots_stopped(&mut resolver, callers);
    assert_outcomes(&files, &observer, callbacks, evidence_failure);
    Ok(())
}

#[test]
fn original_file_projection_stop_remains_fatal_across_file_cache_and_entity_roots() -> TestResult {
    verify_stop(false)
}

#[test]
fn original_file_evidence_failure_never_becomes_a_retained_ordinary_xml_issue() -> TestResult {
    verify_stop(true)
}
