// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_dataforge::DataForge;
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntity};

use super as original_xml;
use super::{Node, Scope};
use crate::loadout_file_trace::FileTrace;
use crate::{
    Entities, LoadoutDocumentKind, LoadoutFileError, LoadoutFiles, LoadoutPath, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutRequest, LoadoutResolver, LoadoutSelection, XmlLoadoutPathSite,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Files(Vec<u8>);

impl LoadoutFiles for Files {
    fn read(&self, _path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        Ok(self.0.clone())
    }
}

#[derive(Default)]
struct Probe(AtomicUsize);

impl LoadoutReadObserver for Probe {
    fn observe(&self, _event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn xml_path(
        &self,
        _selection: &LoadoutSelection,
        _site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ControlFlow::Continue(())
    }
}

fn verify_foreign_parse(binary: bool) -> TestResult {
    let bytes = if binary {
        TestElement::new("Loadout")
            .attribute("loadout", "synthetic/shared.xml")
            .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
    } else {
        b"<Loadout loadout=\"synthetic/shared.xml\"/>".to_vec()
    };
    let database = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(TestEntity::new(1, "Root", "synthetic/root.xml").record())
        .build();
    let forge = DataForge::open(&database)?;
    let probe = Arc::new(Probe::default());
    let resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone());
    let request = LoadoutPath::unbound("synthetic/source.xml");
    let caller = LoadoutRequest::Xml {
        path: request.clone(),
    };
    let files = Files(bytes.clone());
    let document = crate::loadout_file_events::read(
        &files,
        &request,
        &caller,
        LoadoutDocumentKind::XmlLoadout,
        resolver.observation.as_deref(),
        &mut FileTrace::default(),
    )?;
    let path = original_xml::parse(&document, resolver.observation.as_deref(), |root, scope| {
        reject_foreign(scope, &bytes, &probe)?;
        scope.path(root, "loadout")
    })?
    .ok_or("the same-parse positive control is absent")?;
    assert!(
        path.selection().is_some(),
        "the actual original node keeps its authentic selection"
    );
    assert_eq!(
        probe.0.load(Ordering::Relaxed),
        1,
        "only the same-parse getter reaches the observer"
    );
    Ok(())
}

fn reject_foreign(scope: &Scope<'_, '_>, bytes: &[u8], probe: &Probe) -> Result<(), String> {
    let foreign = p4kx_cryxml::read(bytes).map_err(|error| error.to_string())?;
    let rejected = match foreign {
        p4kx_cryxml::Xml::CryXml(foreign) => scope.path(Node::Cry(foreign.root()), "loadout"),
        p4kx_cryxml::Xml::Text(text) => {
            let text = std::str::from_utf8(text).map_err(|error| error.to_string())?;
            let foreign = roxmltree::Document::parse(text).map_err(|error| error.to_string())?;
            scope.path(Node::Text(foreign.root_element()), "loadout")
        }
    };
    assert!(
        rejected.is_err(),
        "equal node coordinates in another parse cannot mint a bound site"
    );
    assert_eq!(
        probe.0.load(Ordering::Relaxed),
        0,
        "foreign nodes never reach the observer"
    );
    Ok(())
}

#[test]
fn equal_text_nodes_from_a_different_parse_are_rejected() -> TestResult {
    verify_foreign_parse(false)
}

#[test]
fn equal_cryxml_nodes_from_a_different_parse_are_rejected() -> TestResult {
    verify_foreign_parse(true)
}
