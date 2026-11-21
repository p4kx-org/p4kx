// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_fixture::{Fixture, Input, TestResult};
use p4kx_assembly::{
    ArchiveFiles, Entities, LoadoutDocumentKind, LoadoutError, LoadoutFileError,
    LoadoutFileObservation, LoadoutFiles, LoadoutReadEvent, LoadoutReadObserver, LoadoutResolver,
};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry, TestLoadout, entity_schema};
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

pub(crate) const PATH: &str = "Data/synthetic/shared.xml";
pub(crate) fn database() -> Vec<u8> {
    let mut source = entity_schema(DataForgeBuilder::new().version(8));
    for (index, guid) in [(0, 0x5731), (1, 0x5732)] {
        source = source.record(
            TestEntity::new(
                guid,
                format!("HealthShip{index}"),
                format!("libs/foundry/records/entities/spaceships/health{index}.xml"),
            )
            .attach_type("NOITEM_Vehicle")
            .loadout(TestLoadout::Xml("synthetic/shared.xml".to_owned()))
            .record(),
        );
    }
    source.build()
}
#[derive(Default)]
struct Probe {
    read: AtomicUsize,
    projected: AtomicUsize,
    physical: AtomicUsize,
    reused: AtomicUsize,
    parse_error: OnceLock<String>,
}
impl LoadoutReadObserver for Probe {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        match event {
            LoadoutReadEvent::FileRead { observation, .. } => {
                self.read.fetch_add(1, Ordering::Relaxed);
                self.physical.fetch_add(
                    usize::from(observation.source().entry_origin().is_some()),
                    Ordering::Relaxed,
                );
            }
            LoadoutReadEvent::FileProjection { observation, .. } => {
                self.projected.fetch_add(1, Ordering::Relaxed);
                let outcome = observation.projection_outcome();
                assert!(
                    outcome.is_some(),
                    "a genuine parse event carries the original result"
                );
                if let Some(Err(reason)) = outcome {
                    let _stored = self.parse_error.set(reason.to_owned());
                }
            }
            LoadoutReadEvent::FileReused { observation, .. } => {
                self.inspect_reuse(observation);
            }
            LoadoutReadEvent::FileDependency { .. }
            | LoadoutReadEvent::FileDependencyRefused { .. }
            | LoadoutReadEvent::InventoryIncomplete
            | LoadoutReadEvent::CanonicalRead { .. }
            | LoadoutReadEvent::ProvidedTree { .. }
            | LoadoutReadEvent::Reused { .. }
            | LoadoutReadEvent::Rejected { .. } => {}
        }
        ControlFlow::Continue(())
    }
}
struct GenericFiles {
    reads: AtomicUsize,
}
impl LoadoutFiles for GenericFiles {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        assert_eq!(
            path, "synthetic/shared.xml",
            "the original authored file selector is supplied"
        );
        self.reads.fetch_add(1, Ordering::Relaxed);
        Ok(b"<broken>".to_vec())
    }
}
pub(crate) fn cache_control(source: &Fixture) -> TestResult {
    admission_control()?;
    let forge = source.entry.open()?;
    let observer = Arc::new(Probe::default());
    let files = GenericFiles {
        reads: AtomicUsize::new(0),
    };
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone())
            .with_files(&files);
    for index in [0, 1] {
        let record = forge.record(index).ok_or("original distinct caller")?;
        assert_eq!(
            record.index(),
            index,
            "the two original callers have distinct physical identities"
        );
        let node = resolver.resolve(&record)?;
        assert!(node.issues().iter().any(|issue| matches!(issue, LoadoutError::XmlLoadout { path, .. } if path == "synthetic/shared.xml")),
            "the actual malformed XML is retained as the root node's precise file issue");
    }
    observer.assert_counts(&files);
    Ok(())
}

fn admission_control() -> TestResult {
    let fixture = Fixture::open(
        &database(),
        Input::Plain,
        vec![TestEntry::stored(PATH, b"<broken>".as_slice())],
    )?;
    let entry = crate::health_origin_entry_source::original(&fixture.archive, PATH)?;
    let actual = fixture
        .archive
        .observed_entry(entry)
        .ok_or("original admitted Entry")?;
    let files = ArchiveFiles::new(&fixture.archive);
    let read = files.read_observed("synthetic/shared.xml");
    let origin = read
        .source()
        .entry_origin()
        .ok_or("genuine same-read Archive admission")?;
    assert!(
        origin.binding().is_same_entry(&actual.binding()),
        "opaque original entry binding"
    );
    assert_eq!(
        read.source().identity().path,
        entry.path(),
        "same original path"
    );
    assert_eq!(
        read.source().identity().entry_index,
        Some(entry.index()),
        "same original position"
    );
    assert_eq!(
        read.outcome().map_err(Clone::clone)?,
        b"<broken>",
        "same admitted read bytes"
    );
    Ok(())
}
impl Probe {
    fn inspect_reuse(&self, observation: &LoadoutFileObservation) {
        self.reused.fetch_add(1, Ordering::Relaxed);
        assert_eq!(
            observation.kind(),
            LoadoutDocumentKind::XmlLoadout,
            "original cached projection"
        );
        assert!(
            observation.source().entry_origin().is_none(),
            "generic admission stays unbound"
        );
        assert!(
            observation.source().identity().entry_index.is_none(),
            "no fabricated Entry position"
        );
        assert!(
            observation.read_outcome().is_ok(),
            "the original bytes were read successfully"
        );
        assert_eq!(
            observation.projection_outcome(),
            self.parse_error.get().map(|reason| Err(reason.as_str())),
            "cached reuse retains the same actual parse failure"
        );
        assert!(
            self.parse_error.get().is_some(),
            "the failed original parse was observed first"
        );
    }

    fn assert_counts(&self, files: &GenericFiles) {
        assert_eq!(
            files.reads.load(Ordering::Relaxed),
            1,
            "one public resolver cache reuses the original failed document read"
        );
        assert_eq!(
            self.read.load(Ordering::Relaxed),
            1,
            "cached reuse does not invent a second completed read"
        );
        assert_eq!(
            self.projected.load(Ordering::Relaxed),
            1,
            "the actual failed parse is observed once"
        );
        assert_eq!(
            self.reused.load(Ordering::Relaxed),
            1,
            "the second actual caller receives the original cached file outcomes",
        );
        assert_eq!(
            self.physical.load(Ordering::Relaxed),
            0,
            "a generic provider supplies no fabricated Archive Entry identity"
        );
    }
}
