// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_assembly::{
    Entities, LoadoutDocumentKind, LoadoutFileError, LoadoutFileObservation, LoadoutFileRead,
    LoadoutFileSource, LoadoutFiles, LoadoutReadEvent, LoadoutReadObserver, LoadoutResolver,
};
use p4kx_extraction::{Archive, DataForge};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout};

use super::FileProofs;
use crate::ExportAccountingError;

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;
pub(super) const SELECTOR: &str = "synthetic/proof.xml";

struct Read {
    operation: LoadoutFileObservation,
    source: LoadoutFileSource,
    kind: LoadoutDocumentKind,
    outcome: Result<(), LoadoutFileError>,
}

pub(super) struct Witness {
    read: Read,
    projected: Option<Result<(), String>>,
    pub(super) original: LoadoutFileObservation,
}

impl Witness {
    pub(super) fn read_into(&self, proofs: &mut FileProofs) -> Result<(), ExportAccountingError> {
        proofs.read(&self.read.operation)
    }

    pub(super) fn populate(&self) -> Result<FileProofs, ExportAccountingError> {
        let mut proofs = FileProofs::default();
        self.read_into(&mut proofs)?;
        if self.projected.is_some() {
            proofs.projected(&self.read.operation)?;
        }
        Ok(proofs)
    }
}

#[derive(Default)]
struct Seen {
    reads: Vec<Read>,
    projections: Vec<Result<(), String>>,
    projected_operations: Vec<LoadoutFileObservation>,
    reused: Vec<LoadoutFileObservation>,
}

#[derive(Default)]
struct Probe(Mutex<Seen>);

impl LoadoutReadObserver for Probe {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        let mut seen = self.0.lock().expect("source witness mutex");
        match event {
            LoadoutReadEvent::FileRead { observation, .. } => seen.reads.push(Read {
                operation: observation.clone(),
                source: observation.source().clone(),
                kind: observation.kind(),
                outcome: observation.read_outcome().map_err(Clone::clone),
            }),
            LoadoutReadEvent::FileProjection { observation, .. } => {
                seen.projected_operations.push(observation.clone());
                seen.projections.push(
                    observation
                        .projection_outcome()
                        .expect("an actual projection callback retains its original result")
                        .map_err(str::to_owned),
                );
            }
            LoadoutReadEvent::FileReused { observation, .. } => {
                seen.reused.push(observation.clone());
            }
            _ => {}
        }
        drop(seen);
        ControlFlow::Continue(())
    }
}

pub(super) fn archive(
    contents: Option<&[u8]>,
) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let mut builder = ArchiveBuilder::new().entry(TestEntry::stored(
        "Data/synthetic/unrelated.xml",
        b"<Loadout/>".as_slice(),
    ));
    if let Some(contents) = contents {
        builder = builder.entry(TestEntry::stored(format!("Data/{SELECTOR}"), contents));
    }
    builder.write_to(&path)?;
    Ok((directory, Archive::open(path)?))
}

pub(super) fn observe(files: &dyn LoadoutFiles) -> Result<Witness, Box<dyn std::error::Error>> {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    assert_eq!(forge.records().count(), 2, "two genuine source callers");
    let probe = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone())
            .with_files(files);
    for record in forge.records() {
        assert_eq!(
            record.tree()?.error_count(),
            0,
            "the actual canonical source is readable"
        );
        resolver.resolve(&record)?;
    }
    drop(resolver);
    finish(probe)
}

fn database() -> Vec<u8> {
    let mut database = p4kx_testkit::entity_schema(DataForgeBuilder::new());
    for guid in [1, 2] {
        database = database.record(
            TestEntity::new(
                guid,
                format!("Caller{guid}"),
                format!("synthetic/{guid}.xml"),
            )
            .loadout(TestLoadout::Xml(SELECTOR.to_owned()))
            .record(),
        );
    }
    database.build()
}

fn finish(probe: Arc<Probe>) -> Result<Witness, Box<dyn std::error::Error>> {
    let probe =
        Arc::try_unwrap(probe).map_err(|_| "the finished resolver releases its observer")?;
    let mut seen = probe.0.into_inner().map_err(|_| "source witness mutex")?;
    assert_eq!(
        seen.reads.len(),
        1,
        "one actual byte read across both original callers"
    );
    assert_eq!(
        seen.reused.len(),
        1,
        "a genuine immutable cache receipt, not a test envelope"
    );
    let read = seen.reads.pop().ok_or("original byte operation")?;
    let original = seen.reused.pop().ok_or("original cache receipt")?;
    assert_eq!(
        original.kind(),
        read.kind,
        "the retained intrinsic operation"
    );
    assert_eq!(
        original.read_outcome(),
        read.outcome.as_ref().copied(),
        "the exact original typed read outcome"
    );
    let projected = projection(&mut seen, &read, &original);
    Ok(Witness {
        read,
        projected,
        original,
    })
}

fn projection(
    seen: &mut Seen,
    read: &Read,
    original: &LoadoutFileObservation,
) -> Option<Result<(), String>> {
    assert_eq!(
        seen.projections.len(),
        usize::from(read.outcome.is_ok()),
        "only the completed read reaches the XML parser"
    );
    assert_eq!(
        seen.projected_operations.len(),
        seen.projections.len(),
        "every real projection has its actual operation handle"
    );
    if let Some(projected_operation) = seen.projected_operations.pop() {
        assert!(
            projected_operation.is_same_operation(&read.operation),
            "the actual projection refers to the same genuine original read"
        );
    }
    assert!(
        original.is_same_operation(&read.operation),
        "the actual cache reuses the precise original operation, not equal metadata"
    );
    assert!(
        original.source().is_same_admission(&read.source),
        "the immutable cache receipt preserves the original read admission"
    );
    let projected = seen.projections.pop();
    assert_eq!(
        original.projection_outcome(),
        projected
            .as_ref()
            .map(|value| value.as_ref().copied().map_err(String::as_str)),
        "the exact original parser outcome"
    );
    projected
}

const VEHICLE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Proof.xml";

struct VehicleFiles<'files> {
    actual: &'files dyn LoadoutFiles,
}

impl LoadoutFiles for VehicleFiles<'_> {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        assert_eq!(
            path, VEHICLE,
            "the actual original vehicle requests its configured file"
        );
        self.actual.read(SELECTOR)
    }

    fn read_observed(&self, path: &str) -> LoadoutFileRead {
        assert_eq!(
            path, VEHICLE,
            "the actual intrinsic reader supplies its configured selector"
        );
        self.actual.read_observed(SELECTOR)
    }
}

pub(super) fn observe_vehicle(
    files: &dyn LoadoutFiles,
) -> Result<Witness, Box<dyn std::error::Error>> {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(3, "Vehicle", "synthetic/vehicle.xml")
                .vehicle(VEHICLE, "")
                .record(),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual vehicle caller")?;
    assert_eq!(
        record.tree()?.error_count(),
        0,
        "the different intrinsic caller is genuinely readable"
    );
    let files = VehicleFiles { actual: files };
    let probe = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone())
            .with_files(&files);
    resolver.resolve(&record)?;
    resolver.resolve(&record)?;
    drop(resolver);
    finish(probe)
}
