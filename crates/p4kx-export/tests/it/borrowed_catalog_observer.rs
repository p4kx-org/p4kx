// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{ArchiveFiles, Entities, LoadoutReadEvent, LoadoutReadObserver};
use p4kx_export::ShipReader;
use p4kx_extraction::{CanonicalReadObserver, DataForgeError, Record, RecordTree, Value};
use p4kx_testkit::TestEntry;

use crate::health_origin_fixture::{Fixture, Input, TestResult};

struct BorrowedProbe<'a> {
    canonical: &'a AtomicUsize,
    files: &'a AtomicUsize,
    reused: &'a AtomicUsize,
}

impl LoadoutReadObserver for BorrowedProbe<'_> {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        match event {
            LoadoutReadEvent::CanonicalRead { .. } | LoadoutReadEvent::ProvidedTree { .. } => {
                self.canonical.fetch_add(1, Ordering::Relaxed);
            }
            LoadoutReadEvent::FileRead { .. } => {
                self.files.fetch_add(1, Ordering::Relaxed);
            }
            LoadoutReadEvent::FileReused { .. } => {
                self.reused.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}

impl CanonicalReadObserver for BorrowedProbe<'_> {
    fn observe(
        &self,
        _: &Record<'_>,
        _: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        self.canonical.fetch_add(1, Ordering::Relaxed);
        ControlFlow::Continue(())
    }
}

#[test]
fn borrowed_observer_uses_real_ship_cache_and_catalog_assembly() -> TestResult {
    let source = Fixture::open(
        &crate::health_origin_loadout_source::database(),
        Input::Plain,
        vec![TestEntry::stored(
            crate::health_origin_loadout_source::PATH,
            b"<Loadout/>".as_slice(),
        )],
    )?;
    let forge = source.entry.open()?;
    let first = forge.record(0).ok_or("actual first original Entity")?;
    let second = forge.record(1).ok_or("actual second original Entity")?;
    let tree = first.tree()?;
    assert!(
        tree.is_from(&first),
        "the original tree belongs to the selected physical source"
    );
    assert!(
        matches!(tree.field("Components"), Some(Value::Array(_))),
        "the actual Entity carries its original component array"
    );
    let files = ArchiveFiles::new(&source.archive);
    let canonical = AtomicUsize::new(0);
    let reads = AtomicUsize::new(0);
    let reused = AtomicUsize::new(0);
    {
        let observer = Arc::new(BorrowedProbe {
            canonical: &canonical,
            files: &reads,
            reused: &reused,
        });
        let mut reader =
            ShipReader::with_observed_entities(Arc::new(Entities::new(&forge)), &files, observer);
        reader
            .read_tree(&first, &tree)
            .map_err(|failure| failure.reason)?;
        reader.read(&second).map_err(|failure| failure.reason)?;
    }
    assert!(
        canonical.load(Ordering::Relaxed) > 0,
        "the real borrowed callback traverses resolver and builder observations"
    );
    assert_eq!(
        reads.load(Ordering::Relaxed),
        1,
        "one original shared XML read populates the actual reader cache"
    );
    assert!(
        reused.load(Ordering::Relaxed) > 0,
        "the second genuine caller uses the original cached document observation"
    );
    check_catalog(&source)
}

fn check_catalog(source: &Fixture) -> TestResult {
    let (_, catalog) = source.diagnostic()?;
    let ships = catalog
        .get("ships")
        .and_then(serde_json::Value::as_array)
        .ok_or("actual catalog ships")?;
    assert_eq!(
        ships.len(),
        2,
        "the real assemble-to-ShipPool-to-LoadoutResolver route consumes the borrowed original capture"
    );
    assert_eq!(
        catalog.get("schemaVersion"),
        Some(&serde_json::json!(3)),
        "catalog schema remains v3"
    );
    Ok(())
}

#[test]
fn borrowed_observer_uses_real_ship_cache_and_catalog_assembly_with_two_workers() -> TestResult {
    use rayon::ThreadPoolBuilder;

    let workers = ThreadPoolBuilder::new().num_threads(2).build()?;
    workers
        .install(|| {
            borrowed_observer_uses_real_ship_cache_and_catalog_assembly()
                .map_err(|error| error.to_string())
        })
        .map_err(Into::into)
}
