// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{LoadoutFileError, LoadoutFiles, LoadoutReadEvent, LoadoutReadObserver};
use p4kx_testkit::{
    CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntity, TestLoadout,
};

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(crate) const ROOT: &str = "synthetic/complete/root.xml";
const SHARED_DOCUMENTS: usize = 6;

pub(crate) struct Files {
    pub(crate) documents: HashMap<String, Vec<u8>>,
    pub(crate) reads: AtomicUsize,
}

impl LoadoutFiles for Files {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.documents
            .get(path)
            .cloned()
            .ok_or(LoadoutFileError::NotFound)
    }
}

#[derive(Default)]
pub(crate) struct Counts {
    pub(crate) reads: AtomicUsize,
    pub(crate) projections: AtomicUsize,
    pub(crate) reused: AtomicUsize,
    pub(crate) incomplete: AtomicUsize,
    pub(crate) dependencies: AtomicUsize,
    pub(crate) refusals: AtomicUsize,
    pub(crate) stop_on_dependency: bool,
    pub(crate) stop_on_refusal: bool,
}

impl LoadoutReadObserver for Counts {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        let stop = (self.stop_on_dependency
            && matches!(&event, LoadoutReadEvent::FileDependency { .. }))
            || (self.stop_on_refusal
                && matches!(&event, LoadoutReadEvent::FileDependencyRefused { .. }));
        let counter = match event {
            LoadoutReadEvent::FileDependency { .. } => &self.dependencies,
            LoadoutReadEvent::FileDependencyRefused { .. } => &self.refusals,
            LoadoutReadEvent::FileRead { .. } => &self.reads,
            LoadoutReadEvent::FileProjection { .. } => &self.projections,
            LoadoutReadEvent::FileReused { .. } => &self.reused,
            LoadoutReadEvent::InventoryIncomplete => &self.incomplete,
            _ => return ControlFlow::Continue(()),
        };
        counter.fetch_add(1, Ordering::Relaxed);
        if stop {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

pub(crate) fn path(index: usize) -> String {
    format!("synthetic/complete/{index}.xml")
}

pub(crate) fn document(paths: impl IntoIterator<Item = String>) -> Vec<u8> {
    let mut includes = TestElement::new("Loadouts");
    for path in paths {
        includes = includes.child(TestElement::new("Loadout").attribute("loadout", path));
    }
    TestElement::new("Loadout")
        .child(includes)
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

pub(crate) fn files() -> Files {
    let mut documents = HashMap::new();
    documents.insert(ROOT.to_owned(), document((0..SHARED_DOCUMENTS).map(path)));
    for index in 0..SHARED_DOCUMENTS {
        documents.insert(path(index), document((0..index).map(path)));
    }
    Files {
        documents,
        reads: AtomicUsize::new(0),
    }
}

pub(crate) fn database() -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "Root", "synthetic/entity.xml")
                .loadout(TestLoadout::Xml(ROOT.to_owned()))
                .record(),
        )
        .build()
}
