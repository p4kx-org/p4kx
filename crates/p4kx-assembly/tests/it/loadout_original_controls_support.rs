// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{
    Entities, LoadoutFileError, LoadoutFileIdentity, LoadoutFiles, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutResolver,
};
use p4kx_dataforge::{DataForge, Record};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestLoadout};

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;
pub(crate) const CONFIGURED: &str = "synthetic/shared.xml";

pub(crate) fn database() -> Vec<u8> {
    let mut source = p4kx_testkit::entity_schema(DataForgeBuilder::new());
    for index in 0..2_u128 {
        source = source.record(
            TestEntity::new(
                index + 1,
                format!("Caller{index}"),
                format!("synthetic/{index}.xml"),
            )
            .loadout(TestLoadout::Xml(CONFIGURED.to_owned()))
            .record(),
        );
    }
    source.build()
}

pub(crate) fn callers<'f>(
    forge: &'f DataForge<'f>,
) -> Result<[Record<'f>; 2], Box<dyn std::error::Error>> {
    let first = forge
        .record(0)
        .ok_or("the first original caller is absent")?;
    let second = forge
        .record(1)
        .ok_or("the second original caller is absent")?;
    assert_ne!(
        first.guid(),
        second.guid(),
        "the callers have distinct original GUIDs"
    );
    assert_eq!(
        first.index(),
        0,
        "the first caller retains its physical position"
    );
    assert_eq!(
        second.index(),
        1,
        "the second caller retains its physical position"
    );
    for record in [first, second] {
        assert_eq!(
            record.type_name()?,
            "EntityClassDefinition",
            "the real source is an Entity"
        );
        let tree = record.tree()?;
        assert!(
            tree.field("Components").is_some(),
            "the real source supplies its Components"
        );
    }
    Ok([first, second])
}

#[derive(Default)]
pub(crate) struct GenericFiles {
    pub(crate) reads: AtomicUsize,
    identities: AtomicUsize,
}

impl LoadoutFiles for GenericFiles {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        assert_eq!(
            path, CONFIGURED,
            "the reader gets the exact original authored path"
        );
        self.reads.fetch_add(1, Ordering::Relaxed);
        Ok(b"<broken>".to_vec())
    }

    fn identity(&self, path: &str) -> Option<LoadoutFileIdentity<'_>> {
        assert_eq!(
            path, CONFIGURED,
            "the legacy descriptor uses the configured path"
        );
        self.identities.fetch_add(1, Ordering::Relaxed);
        Some(LoadoutFileIdentity {
            entry_index: Some(0),
            path: CONFIGURED,
        })
    }
}

#[derive(Default)]
pub(crate) struct Probe {
    pub(crate) reads: AtomicUsize,
    pub(crate) projections: AtomicUsize,
    pub(crate) stop_on_read: bool,
}

impl LoadoutReadObserver for Probe {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        match event {
            LoadoutReadEvent::FileRead { .. } => {
                self.reads.fetch_add(1, Ordering::Relaxed);
                if self.stop_on_read {
                    return ControlFlow::Break(());
                }
            }
            LoadoutReadEvent::FileProjection { .. } => {
                self.projections.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}

pub(crate) fn resolve_shared(observed: bool) -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let originals = callers(&forge)?;
    let files = GenericFiles::default();
    let probe = Arc::new(Probe::default());
    let entities = Arc::new(Entities::new(&forge));
    let mut resolver = if observed {
        LoadoutResolver::with_observed_entities(entities, probe.clone())
    } else {
        LoadoutResolver::with_entities(entities)
    }
    .with_files(&files);
    for record in originals {
        let node = resolver.resolve(&record)?;
        assert!(
            node.issues().iter().any(|issue| matches!(issue,
                p4kx_assembly::LoadoutError::XmlLoadout { path, .. } if path == CONFIGURED)),
            "each caller keeps the precise original malformed-document issue"
        );
    }
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        1,
        "the shared malformed XML is read once"
    );
    let expected = usize::from(observed);
    assert_eq!(
        probe.reads.load(Ordering::Relaxed),
        expected,
        "cache reuse invents no physical read"
    );
    assert_eq!(
        probe.projections.load(Ordering::Relaxed),
        expected,
        "cache reuse invents no parse"
    );
    Ok(())
}
