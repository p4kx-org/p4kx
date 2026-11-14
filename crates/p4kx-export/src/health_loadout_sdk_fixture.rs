// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read;
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_assembly::{
    Entities, LoadoutError, LoadoutFileError, LoadoutFileObservation, LoadoutFiles, LoadoutPath,
    LoadoutPathObserver, LoadoutReadEvent, LoadoutReadObserver, LoadoutRequest, LoadoutResolver,
    LoadoutSelection, XmlLoadoutPathSite,
};
use p4kx_extraction::{Archive, CancelToken, DataForge, Record, RecordTree};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry, TestLoadout};

use crate::Kind;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture};
use crate::export_ledger::ExportLedger;
use crate::loadout::ledger::Capture;

pub(super) const ROOT: &str = "synthetic/sdk-root.xml";
pub(super) const CHILD: &str = "synthetic/sdk-child.xml";
pub(super) const ROOT_XML: &[u8] =
    b"<Loadout><Loadouts><Loadout loadout=\"synthetic/sdk-child.xml\"/></Loadouts></Loadout>";
pub(super) const CHILD_XML: &[u8] = b"<Loadout/>";
pub(super) const INCLUDE_NODE: u32 = 3;
pub(super) type TestResult = Result<(), Box<dyn Error>>;

pub(super) fn database() -> Vec<u8> {
    let mut builder = p4kx_testkit::entity_schema(DataForgeBuilder::new());
    for id in [1, 2] {
        builder = builder.record(
            TestEntity::new(id, format!("Sdk{id}"), format!("synthetic/caller{id}.xml"))
                .loadout(TestLoadout::Xml(ROOT.to_owned()))
                .record(),
        );
    }
    builder.build()
}

pub(super) fn entries(child: Option<&[u8]>) -> Vec<TestEntry> {
    let mut entries = vec![TestEntry::stored(format!("Data/{ROOT}"), ROOT_XML)];
    if let Some(child) = child {
        entries.push(TestEntry::stored(format!("Data/{CHILD}"), child));
    }
    entries
}

pub(super) fn original(
    archive: &Archive,
    forge: &DataForge<'_>,
    child: Option<&[u8]>,
) -> TestResult {
    assert_eq!(forge.records().count(), 2, "two original caller records");
    for record in forge.records() {
        let tree = record.tree()?;
        assert!(tree.is_from(&record), "actual original Record/Tree pair");
        assert_eq!(
            tree.error_count(),
            0,
            "original source has no decoder errors"
        );
    }
    for (path, wanted) in [(ROOT, Some(ROOT_XML)), (CHILD, child)] {
        let name = format!("Data/{path}");
        let entry = archive.entries().iter().find(|entry| entry.path() == name);
        if let Some(wanted) = wanted {
            let mut bytes = Vec::new();
            archive
                .open_entry(entry.ok_or("original XML entry")?)?
                .read_to_end(&mut bytes)?;
            assert_eq!(
                bytes, wanted,
                "exact original archive XML bytes before receiver"
            );
            if path == ROOT {
                original_include(&bytes)?;
            }
        } else {
            assert!(entry.is_none(), "the child genuinely has no archive entry");
        }
    }
    Ok(())
}

fn original_include(bytes: &[u8]) -> TestResult {
    let text = std::str::from_utf8(bytes)?;
    let document = roxmltree::Document::parse(text)?;
    let node = document
        .descendants()
        .find(|node| node.attribute("loadout").is_some())
        .ok_or("original Include XML attribute")?;
    assert_eq!(
        node.id().get(),
        INCLUDE_NODE,
        "independent original decoder node index before callbacks"
    );
    assert_eq!(
        node.attribute("loadout"),
        Some(CHILD),
        "actual original authored Include value"
    );
    Ok(())
}

#[derive(Clone)]
pub(super) struct Edge {
    pub(super) parent: LoadoutFileObservation,
    pub(super) child: LoadoutFileObservation,
    pub(super) request: LoadoutPath,
    pub(super) dependency: LoadoutRequest,
}

#[derive(Clone)]
pub(super) struct ReadContext {
    pub(super) original: LoadoutFileObservation,
    pub(super) request: LoadoutPath,
    pub(super) caller: LoadoutRequest,
}

#[derive(Default)]
pub(super) struct Seen {
    pub(super) first: Option<ReadContext>,
    pub(super) reads: Vec<LoadoutFileObservation>,
    pub(super) projections: usize,
    pub(super) edges: Vec<Edge>,
    pub(super) reused: usize,
    pub(super) paths: Vec<(u32, AuthoredEntryPath)>,
}

pub(super) struct Probe<'input> {
    input: &'input OriginalCatalogInput<'input, 'input>,
    pub(super) capture: Arc<Capture<'input>>,
    pub(super) seen: Mutex<Seen>,
}

impl<'input> Probe<'input> {
    pub(super) fn new(
        input: &'input OriginalCatalogInput<'input, 'input>,
        cancel: Option<&CancelToken>,
    ) -> Self {
        Self {
            input,
            capture: Arc::new(Capture::for_input(
                input,
                &ExportLedger::default(),
                cancel,
                Kind::Loadouts,
            )),
            seen: Mutex::new(Seen::default()),
        }
    }
}

impl LoadoutReadObserver for Probe<'_> {
    fn with_record_paths(
        &self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        self.capture.with_record_paths(record, tree, parse)
    }

    fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        let flow = self.capture.xml_path(selection, site);
        if flow.is_continue() {
            let recipe = EntryCapture::for_input(self.input)
                .loadout_xml(site)
                .expect("the same original site remains valid within its actual callback");
            self.seen
                .lock()
                .expect("SDK source observations")
                .paths
                .push((site.node_index(), recipe));
        }
        flow
    }

    fn derived_path(
        &self,
        selection: &LoadoutSelection,
        inputs: &[&LoadoutPath],
        value: &str,
    ) -> ControlFlow<()> {
        self.capture.derived_path(selection, inputs, value)
    }

    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        let mut seen = self.seen.lock().expect("SDK source observations");
        match &event {
            LoadoutReadEvent::FileRead {
                observation,
                request,
                caller,
            } => {
                seen.reads.push((*observation).clone());
                seen.first.get_or_insert_with(|| ReadContext {
                    original: (*observation).clone(),
                    request: (*request).clone(),
                    caller: (*caller).clone(),
                });
            }
            LoadoutReadEvent::FileProjection { .. } => seen.projections += 1,
            LoadoutReadEvent::FileReused { .. } => seen.reused += 1,
            LoadoutReadEvent::FileDependency {
                parent,
                observation,
                request,
                dependency,
            } => {
                seen.edges.push(Edge {
                    parent: (*parent).clone(),
                    child: (*observation).clone(),
                    request: (*request).clone(),
                    dependency: (*dependency).clone(),
                });
            }
            _ => {}
        }
        drop(seen);
        self.capture.observe(event)
    }
}

pub(super) fn resolve<'input>(
    forge: &'input DataForge<'input>,
    files: &'input dyn LoadoutFiles,
    probe: Arc<Probe<'input>>,
    twice: bool,
) -> Result<(), LoadoutError> {
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(forge)), probe)
            .with_files(files);
    let count = if twice { 2 } else { 1 };
    for index in 0..count {
        resolver.resolve(&forge.record(index).expect("the original caller exists"))?;
    }
    Ok(())
}

pub(super) struct Generic;

impl LoadoutFiles for Generic {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        match path {
            ROOT => Ok(ROOT_XML.to_vec()),
            CHILD => Ok(CHILD_XML.to_vec()),
            _ => Err(LoadoutFileError::NotFound),
        }
    }
}
