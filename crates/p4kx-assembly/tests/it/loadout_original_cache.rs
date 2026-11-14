// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use p4kx_archive::{Archive, EntryOrigin};
use p4kx_assembly::{
    ArchiveFiles, Entities, LoadoutFileError, LoadoutFileIdentity, LoadoutFileObservation,
    LoadoutFileRead, LoadoutFileSource, LoadoutFiles, LoadoutReadEvent, LoadoutReadObserver,
    LoadoutResolver,
};
use p4kx_dataforge::DataForge;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::loadout_original_controls_support::{self as support, TestResult};

struct Provider<'archive> {
    files: ArchiveFiles<'archive>,
    reads: AtomicUsize,
    identities: AtomicUsize,
}

impl LoadoutFiles for Provider<'_> {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.files.read(path)
    }

    fn read_observed(&self, path: &str) -> LoadoutFileRead {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.files.read_observed(path)
    }

    fn identity(&self, _path: &str) -> Option<LoadoutFileIdentity<'_>> {
        self.identities.fetch_add(1, Ordering::Relaxed);
        Some(LoadoutFileIdentity {
            entry_index: Some(1),
            path: "Data/synthetic/decoy.xml",
        })
    }
}

#[derive(Default)]
struct Seen {
    read: Option<EntryOrigin>,
    projections: usize,
    reuses: usize,
    read_error: Option<LoadoutFileError>,
    projected_error: Option<String>,
}

#[derive(Default)]
struct Probe(Mutex<Seen>);

fn same_read(original: &EntryOrigin, current: &EntryOrigin) -> bool {
    original
        .input_origin()
        .is_same_input(current.input_origin())
        && std::ptr::eq(original.entry(), current.entry())
}

impl Probe {
    fn read(
        &self,
        source: &LoadoutFileSource,
        outcome: Result<(), &LoadoutFileError>,
    ) -> TestResult {
        let original = source
            .entry_origin()
            .ok_or("an actual Archive read retains its original admission")?;
        let mut seen = self
            .0
            .lock()
            .map_err(|_| "the test source observation mutex is not poisoned")?;
        assert!(
            seen.read.is_none(),
            "one cached source performs exactly one physical read"
        );
        seen.read = Some(original.clone());
        seen.read_error = outcome.err().cloned();
        drop(seen);
        Ok(())
    }

    fn projected(&self, source: &LoadoutFileSource, outcome: Result<(), &str>) -> TestResult {
        let original = source
            .entry_origin()
            .ok_or("projection retains the actual read's original source")?;
        let mut seen = self
            .0
            .lock()
            .map_err(|_| "the test source observation mutex is not poisoned")?;
        assert!(
            same_read(
                seen.read
                    .as_ref()
                    .ok_or("projection follows its actual read")?,
                original
            ),
            "projection borrows the same original snapshot, without a new source lookup"
        );
        seen.projections += 1;
        seen.projected_error = outcome.err().map(str::to_owned);
        drop(seen);
        Ok(())
    }

    fn reused(&self, observation: &LoadoutFileObservation) -> TestResult {
        let original = observation
            .source()
            .entry_origin()
            .ok_or("the cache retains original admission")?;
        let mut seen = self
            .0
            .lock()
            .map_err(|_| "the test source observation mutex is not poisoned")?;
        assert!(
            same_read(
                seen.read
                    .as_ref()
                    .ok_or("cache reuse follows its actual read")?,
                original
            ),
            "cache reuse retains the first operation's actual original snapshot"
        );
        assert_eq!(
            observation.read_outcome().err(),
            seen.read_error.as_ref(),
            "cache replay preserves the exact actual read failure"
        );
        let projected = observation.projection_outcome();
        assert_eq!(
            projected.is_some(),
            seen.read_error.is_none(),
            "only a completed original read reaches the actual parser"
        );
        let reason = projected.and_then(Result::err);
        assert_eq!(
            reason,
            seen.projected_error.as_deref(),
            "cache replay retains the exact actual projection outcome"
        );
        seen.reuses += 1;
        drop(seen);
        Ok(())
    }
}

impl LoadoutReadObserver for Probe {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        let result = match event {
            LoadoutReadEvent::FileRead { observation, .. } => {
                self.read(observation.source(), observation.read_outcome())
            }
            LoadoutReadEvent::FileProjection { observation, .. } => {
                observation.projection_outcome().map_or_else(
                    || Err("a genuine projection callback retains its actual parse outcome".into()),
                    |outcome| self.projected(observation.source(), outcome),
                )
            }
            LoadoutReadEvent::FileReused { observation, .. } => self.reused(observation),
            _ => Ok(()),
        };
        assert!(
            result.is_ok(),
            "original source observation failed: {result:?}"
        );
        ControlFlow::Continue(())
    }
}

fn source_archive(
    directory: &std::path::Path,
    encrypted: bool,
) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = directory.join("Data.p4k");
    let entry = TestEntry::stored(
        format!("Data/{}", support::CONFIGURED),
        b"<broken>".to_vec(),
    );
    let entry = if encrypted { entry.encrypted() } else { entry };
    ArchiveBuilder::new()
        .entry(entry)
        .entry(TestEntry::stored(
            "Data/synthetic/decoy.xml",
            b"<Loadout/>".to_vec(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let original = archive
        .entries()
        .first()
        .ok_or("the actual original source is absent")?;
    assert_eq!(
        original.path(),
        format!("Data/{}", support::CONFIGURED),
        "the original source is not the decoy"
    );
    Ok(archive)
}

fn verify_seen(archive: &Archive, seen: &Seen, encrypted: bool) -> TestResult {
    let original = archive
        .entries()
        .first()
        .ok_or("the original admitted entry is absent")?;
    let retained = seen
        .read
        .as_ref()
        .ok_or("no original source was observed")?;
    assert!(
        archive.owns_origin(retained),
        "the retained input is owned by this actual Archive"
    );
    assert_eq!(
        retained.entry().index(),
        0,
        "the original admitted slot survives the false identity provider"
    );
    assert_eq!(
        retained.entry().path(),
        original.path(),
        "the original admitted path survives the false identity provider"
    );
    assert_eq!(
        seen.read_error.is_some(),
        encrypted,
        "the actual missing-key case fails before parsing"
    );
    assert_eq!(
        seen.projections,
        usize::from(!encrypted),
        "only a completed byte read is parsed"
    );
    assert_eq!(
        seen.reuses, 1,
        "the second actual caller reuses original outcomes once"
    );
    Ok(())
}

fn verify(encrypted: bool) -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = source_archive(directory.path(), encrypted)?;
    let database = support::database();
    let forge = DataForge::open(&database)?;
    let callers = support::callers(&forge)?;
    let files = Provider {
        files: ArchiveFiles::new(&archive),
        reads: AtomicUsize::new(0),
        identities: AtomicUsize::new(0),
    };
    let observer = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone())
            .with_files(&files);
    for caller in callers {
        let node = resolver.resolve(&caller)?;
        assert!(
            node.issues().iter().any(|issue| matches!(issue,
            p4kx_assembly::LoadoutError::XmlLoadout { path, .. } if path == support::CONFIGURED)),
            "both actual callers retain the same precise original file failure"
        );
    }
    let seen = observer
        .0
        .lock()
        .map_err(|_| "the final test observation mutex is not poisoned")?;
    verify_seen(&archive, &seen, encrypted)?;
    drop(seen);
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        1,
        "the cache performs no second physical read"
    );
    assert_eq!(
        files.identities.load(Ordering::Relaxed),
        0,
        "read, projection and reuse never re-look-up a descriptor"
    );
    Ok(())
}

#[test]
fn original_loadout_cache_preserves_actual_parse_failure_source_without_identity_lookup()
-> TestResult {
    verify(false)
}

#[test]
fn original_loadout_cache_preserves_actual_open_failure_source_without_projection() -> TestResult {
    verify(true)
}
