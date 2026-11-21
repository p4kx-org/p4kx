// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_archive::Archive;
use p4kx_dataforge::DataForge;
use p4kx_extraction::SourceLocation;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry};

use crate::{
    ArchiveFiles, Entities, LoadoutFileObservation, LoadoutReadEvent, LoadoutReadObserver,
    LoadoutRequest, LoadoutResolver, VehicleImplementation, VehicleXmlSite,
};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub(super) const BASE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/scope.xml";
pub(super) const PATCH: &str =
    "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/scope_Observed.xml";

pub(super) struct Observer {
    enabled: bool,
    stop_on_use: bool,
    pub(super) parsed: Mutex<Vec<LoadoutFileObservation>>,
    pub(super) read_paths: Mutex<Vec<String>>,
    pub(super) callers: Mutex<Vec<(u32, LoadoutRequest)>>,
}

impl Observer {
    pub(super) fn stopping() -> Self {
        Self {
            stop_on_use: true,
            ..Self::new(true)
        }
    }

    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            stop_on_use: false,
            parsed: Mutex::new(Vec::new()),
            read_paths: Mutex::new(Vec::new()),
            callers: Mutex::new(Vec::new()),
        }
    }
}

impl LoadoutReadObserver for Observer {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        match event {
            LoadoutReadEvent::FileProjection { observation, .. } => {
                self.parsed
                    .lock()
                    .expect("original parse witness")
                    .push(observation.clone());
            }
            LoadoutReadEvent::FileRead { observation, .. } => {
                self.read_paths
                    .lock()
                    .expect("original file-read witness")
                    .push(observation.source().identity().path.to_owned());
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn observes_vehicle_totals(&self) -> bool {
        self.enabled
    }

    fn vehicle_used(
        &self,
        record_index: u32,
        _implementation: &VehicleImplementation,
        caller: &LoadoutRequest,
    ) -> ControlFlow<()> {
        self.callers
            .lock()
            .expect("current caller witness")
            .push((record_index, caller.clone()));
        if self.stop_on_use {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

pub(super) fn archive(
    base: &[u8],
    patch: Option<&[u8]>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let archive_path = directory.path().join("Data.p4k");
    let mut builder = ArchiveBuilder::new().entry(TestEntry::stored(format!("Data/{BASE}"), base));
    if let Some(patch) = patch {
        builder = builder.entry(TestEntry::stored(format!("Data/{PATCH}"), patch).encrypted());
    }
    builder
        .entry(TestEntry::stored("Data/unrelated.xml", b"<Unused/>".as_slice()).encrypted())
        .write_to(&archive_path)?;
    Ok((
        directory,
        Archive::open_with_key(archive_path, Some(p4kx_testkit::test_key()))?,
    ))
}

pub(super) fn database(modification: &str) -> Vec<u8> {
    database_with_modifications(modification, modification)
}

pub(super) fn database_with_modifications(first: &str, second: &str) -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(71, "ScopeFixture", "synthetic/scope-a.xml")
                .vehicle(BASE, first)
                .record(),
        )
        .record(
            TestEntity::new(71, "ScopeFixture", "synthetic/scope-b.xml")
                .vehicle(BASE.to_ascii_lowercase(), second)
                .record(),
        )
        .build()
}

pub(super) fn with_vehicle(
    base: &[u8],
    patch: Option<&[u8]>,
    modification: &str,
    enabled: bool,
    check: impl FnOnce(&VehicleImplementation, &Observer) -> TestResult,
) -> TestResult {
    let (_directory, archive) = archive(base, patch)?;
    with_archive(&archive, modification, enabled, check)
}

pub(super) fn with_archive(
    archive: &Archive,
    modification: &str,
    enabled: bool,
    check: impl FnOnce(&VehicleImplementation, &Observer) -> TestResult,
) -> TestResult {
    let files = ArchiveFiles::new(archive);
    let bytes = database(modification);
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("synthetic source Record")?;
    let observer = Arc::new(Observer::new(enabled));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone())
            .with_files(&files);
    let node = resolver.resolve(&record)?;
    check(
        node.vehicle().ok_or("synthetic vehicle implementation")?,
        &observer,
    )
}

pub(super) fn assert_original_sites(
    vehicle: &VehicleImplementation,
    observer: &Observer,
) -> TestResult {
    assert!(
        observer
            .read_paths
            .lock()
            .map_err(|_| "poisoned file-read witness")?
            .iter()
            .all(|path| !path.ends_with("unrelated.xml"))
    );
    let evidence = vehicle
        .aggregate_evidence()
        .ok_or("opt-in aggregate source scope")?;
    assert!(evidence.has_complete_coordinates());
    let parsed = observer
        .parsed
        .lock()
        .map_err(|_| "poisoned parse witness")?;
    let sites = evidence.selectors().collect::<Vec<_>>();
    for (index, site) in sites.iter().enumerate() {
        assert!(
            !sites[..index]
                .iter()
                .any(|earlier| earlier.file().is_same_operation(site.file())
                    && earlier.location() == site.location())
        );
        assert!(site.location().is_some());
        assert!(matches!(site.file().projection_outcome(), Some(Ok(()))));
        assert!(
            parsed
                .iter()
                .any(|parse| parse.is_same_operation(site.file()))
        );
        assert!(site.file().source().entry_origin().is_some());
    }
    drop(parsed);
    assert!(
        evidence.selectors().next().is_some(),
        "the actual root was observed"
    );
    Ok(())
}

pub(super) fn base_fragment<'bytes>(
    site: &VehicleXmlSite,
    bytes: &'bytes [u8],
) -> Option<&'bytes str> {
    let source = site.file().source();
    if source.identity().entry_index != Some(0)
        || source.identity().path.strip_prefix("Data/") != Some(BASE)
        || source.entry_origin().is_none()
    {
        return None;
    }
    let SourceLocation::Subresource {
        namespace, indices, ..
    } = site.location()?
    else {
        return None;
    };
    if namespace != "textXmlAttribute" {
        return None;
    }
    let [start, end] = indices.as_slice() else {
        return None;
    };
    let start = usize::try_from(*start).ok()?;
    let end = usize::try_from(*end).ok()?;
    std::str::from_utf8(bytes.get(start..end)?).ok()
}

pub(crate) fn with_original_root(
    base: &[u8],
    check: impl FnOnce(&mut crate::markup::Element, &mut crate::VehicleAggregateEvidence) -> TestResult,
) -> TestResult {
    let (_directory, archive) = archive(base, None)?;
    let files = ArchiveFiles::new(&archive);
    let bytes = database("");
    let forge = DataForge::open(&bytes)?;
    let observer = Arc::new(Observer::new(true));
    let resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer);
    let path = crate::LoadoutPath::unbound(BASE);
    let caller = LoadoutRequest::Vehicle {
        definition: path.clone(),
        modification: crate::LoadoutPath::unbound(""),
        class_name: None,
    };
    let mut trace = crate::loadout_file_trace::FileTrace::default();
    let document = crate::loadout_file_events::read(
        &files,
        &path,
        &caller,
        crate::LoadoutDocumentKind::VehicleImplementation,
        resolver.observation.as_deref(),
        &mut trace,
    )?;
    let (mut root, _, mut evidence) = crate::vehicle::file_read::base(
        BASE,
        document,
        &mut crate::markup::Budget::new(base.len()),
        "",
        resolver.observation.as_deref(),
    )?;
    check(
        &mut root,
        evidence.as_mut().ok_or("opt-in aggregate scope")?,
    )
}
