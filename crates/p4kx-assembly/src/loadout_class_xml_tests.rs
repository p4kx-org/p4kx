// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_dataforge::DataForge;
use p4kx_testkit::{
    CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntity, TestLoadout,
};

use crate::loadout_file_events::Document;
use crate::loadout_file_trace::FileTrace;
use crate::loadout_xml::{self, Scope};
use crate::{
    Entities, LoadoutDocumentKind, LoadoutError, LoadoutFileError, LoadoutFiles, LoadoutPath,
    LoadoutRequest, LoadoutResolver,
};

use super::test_source::{Recorder, Stop, TestResult};

#[path = "loadout_class_xml_assertions.rs"]
mod assertions;

use self::assertions::verify_xml;

struct Files {
    bytes: Vec<u8>,
    reads: AtomicUsize,
}
impl Files {
    const fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            reads: AtomicUsize::new(0),
        }
    }
}
impl LoadoutFiles for Files {
    fn read(&self, _: &str) -> Result<Vec<u8>, LoadoutFileError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        Ok(self.bytes.clone())
    }
}

fn bytes(binary: bool) -> Vec<u8> {
    if binary {
        TestElement::new("Loadout")
            .child(
                TestElement::new("Items")
                    .child(
                        TestElement::new("Item")
                            .attribute("portName", "one")
                            .attribute("itemName", "Self"),
                    )
                    .child(
                        TestElement::new("Item")
                            .attribute("portName", "two")
                            .attribute("itemName", "Self"),
                    ),
            )
            .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
    } else {
        b"<Loadout><Items><Item portName=\"one\" itemName=\"Self\"/><Item portName=\"two\" itemName=\"Self\"/></Items></Loadout>".to_vec()
    }
}

fn original(bytes: &[u8]) -> Result<(), String> {
    loadout_xml::parse_unbound(bytes, |root, _| {
        assert_eq!(
            root.name(),
            "Loadout",
            "the independently decoded source root"
        );
        let items = root
            .children()
            .into_iter()
            .find(|node| node.name() == "Items")
            .ok_or("original Items")?;
        let entries = items.children();
        assert_eq!(
            entries.len(),
            2,
            "both original XML slots precede producer interpretation"
        );
        for (item, port) in entries.into_iter().zip(["one", "two"]) {
            assert_eq!(item.name(), "Item", "the original element name");
            assert_eq!(
                item.attribute("portName"),
                Some(port),
                "the original sibling order"
            );
            assert_eq!(
                item.attribute("itemName"),
                Some("Self"),
                "the exact original class attribute"
            );
            assert!(
                item.attribute("className").is_none(),
                "the original grammar does not rename itemName"
            );
        }
        Ok(())
    })
}

fn xml_case(binary: bool) -> TestResult {
    let files = Files::new(bytes(binary));
    original(&files.bytes)?;
    let encoded = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "Self", "synthetic/root.xml")
                .loadout(TestLoadout::Xml("synthetic/loadout.xml".to_owned()))
                .record(),
        )
        .build();
    let forge = DataForge::open(&encoded)?;
    let record = forge.record(0).ok_or("actual XML referrer")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record) && tree.error_count() == 0,
        "the original canonical referrer is authentic and readable"
    );
    let recorder = Arc::new(Recorder::new(&forge, Stop::Never));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), recorder.clone())
            .with_files(&files);
    let node = resolver.resolve_tree(&record, &tree)?;
    let again = resolver.resolve(&record)?;
    assert_eq!(node, again, "XML cache reuse preserves semantic resolution");
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        1,
        "the original XML operation is not repeated"
    );
    let log = recorder.log.lock().map_err(|_| "source log")?;
    verify_xml(&node, &log, binary)?;
    drop(log);
    Ok(())
}

#[test]
fn text_and_cryxml_class_getters_keep_original_nodes_selected_records_and_cache_pairs() -> TestResult
{
    xml_case(false)?;
    xml_case(true)
}

fn foreign(scope: &Scope<'_, '_>, bytes: &[u8]) -> Result<(), String> {
    loadout_xml::parse_unbound(bytes, |root, _| {
        assert!(
            scope.class_name(root, "itemName").is_err(),
            "equal original metadata from another actual parse cannot bind a class source"
        );
        Ok(())
    })
}

fn document(files: &Files, resolver: &LoadoutResolver<'_>) -> Result<Document, LoadoutFileError> {
    let path = LoadoutPath::unbound("synthetic/source.xml");
    crate::loadout_file_events::read(
        files,
        &path,
        &LoadoutRequest::Xml { path: path.clone() },
        LoadoutDocumentKind::XmlLoadout,
        resolver.observation.as_deref(),
        &mut FileTrace::default(),
    )
}

#[test]
fn xml_class_source_rejects_foreign_text_and_binary_parse_before_minting_selection() -> TestResult {
    for binary in [false, true] {
        let source = if binary {
            TestElement::new("Item")
                .attribute("itemName", "Self")
                .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
        } else {
            b"<Item itemName=\"Self\"/>".to_vec()
        };
        let encoded = p4kx_testkit::entity_schema(DataForgeBuilder::new())
            .record(TestEntity::new(1, "Self", "synthetic/root.xml").record())
            .build();
        let forge = DataForge::open(&encoded)?;
        let recorder = Arc::new(Recorder::new(&forge, Stop::Never));
        let resolver = LoadoutResolver::with_observed_entities(
            Arc::new(Entities::new(&forge)),
            recorder.clone(),
        );
        let document = document(&Files::new(source.clone()), &resolver)?;
        let class =
            loadout_xml::parse(&document, resolver.observation.as_deref(), |root, scope| {
                foreign(scope, &source)?;
                assert!(
                    recorder.log.lock().expect("source log").xml.is_empty(),
                    "foreign parse rejection precedes any source callback"
                );
                scope.class_name(root, "itemName")
            })?
            .ok_or("same-parse class positive")?;
        assert!(
            class.selection().is_some(),
            "the genuine same-parse positive retains its operation"
        );
        assert_eq!(
            recorder.log.lock().map_err(|_| "source log")?.xml.len(),
            1,
            "only the genuine original reaches the receiver"
        );
    }
    Ok(())
}

#[test]
fn xml_class_getter_preserves_the_original_first_local_name_attribute_contract() -> TestResult {
    let bytes = b"<Item xmlns:x=\"urn:test\" x:itemName=\"wrong\" itemName=\"Self\"/>";
    let text = std::str::from_utf8(bytes)?;
    let original = roxmltree::Document::parse(text)?;
    let first = original
        .root_element()
        .attribute_node("itemName")
        .ok_or("the original winning attribute")?;
    assert_eq!(
        first.namespace(),
        Some("urn:test"),
        "the actual original getter selects the first local-name match"
    );
    assert_eq!(
        first.value(),
        "wrong",
        "the original first attribute value is independently fixed"
    );
    assert_eq!(
        original
            .root_element()
            .attributes()
            .find(|attribute| attribute.namespace().is_none())
            .map(|attribute| attribute.value()),
        Some("Self"),
        "the independent unqualified attribute is distinct and later"
    );
    loadout_xml::parse_unbound(bytes, |root, scope| {
        assert_eq!(
            root.attribute("itemName"),
            Some("wrong"),
            "the unchanged original Node accessor delegates the first local-name rule"
        );
        assert_eq!(
            scope
                .class_name(root, "itemName")?
                .map(|class| class.value().to_owned()),
            Some("wrong".to_owned()),
            "the shared source-site implementation preserves the actual original winner"
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn xml_class_observer_refusal_stops_before_copy_later_items_or_selected_record() -> TestResult {
    let files = Files::new(bytes(false));
    original(&files.bytes)?;
    let encoded = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "Self", "synthetic/root.xml")
                .loadout(TestLoadout::Xml("synthetic/loadout.xml".to_owned()))
                .record(),
        )
        .build();
    let forge = DataForge::open(&encoded)?;
    let record = forge.record(0).ok_or("actual source referrer")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record) && tree.error_count() == 0,
        "the genuine source is readable before resolution"
    );
    let recorder = Arc::new(Recorder::new(&forge, Stop::Xml));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), recorder.clone())
            .with_files(&files);
    assert!(
        matches!(
            resolver.resolve_tree(&record, &tree),
            Err(LoadoutError::ObservationStopped)
        ),
        "a failed XML getter cannot publish a complete prefix"
    );
    assert!(
        matches!(
            resolver.resolve(&record),
            Err(LoadoutError::ObservationStopped)
        ),
        "cache reuse retains the first refusal"
    );
    let log = recorder.log.lock().map_err(|_| "source log")?;
    assert_eq!(
        (log.xml.len(), log.chosen.len()),
        (1, 0),
        "later actual XML slots and target copies are not visited"
    );
    drop(log);
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        1,
        "refusal causes no repeated source read"
    );
    Ok(())
}

fn original_attribute_slots(bytes: &[u8]) -> TestResult {
    let text = std::str::from_utf8(bytes)?;
    let original = roxmltree::Document::parse(text)?;
    let root = original.root_element();
    assert_eq!(
        root.id().get(),
        1,
        "the original decoder's root element index is independently fixed"
    );
    let attributes: Vec<_> = root
        .attributes()
        .map(|attribute| (attribute.name(), attribute.namespace(), attribute.value()))
        .collect();
    assert_eq!(
        attributes,
        [
            ("portName", None, "one"),
            ("itemName", Some("urn:test"), "Self"),
            ("itemName", None, "other"),
            ("neighbor", None, "after"),
        ],
        "the complete original attribute order includes an earlier slot and the distinct same-name neighbor"
    );
    assert_eq!(
        root.attribute("itemName"),
        Some("Self"),
        "the original accessor selects exactly the first local-name match"
    );
    Ok(())
}

#[test]
fn xml_class_site_identifies_the_actual_original_attribute_slot_and_neighbors() -> TestResult {
    let bytes = b"<Item xmlns:x=\"urn:test\" portName=\"one\" x:itemName=\"Self\" itemName=\"other\" neighbor=\"after\"/>".to_vec();
    original_attribute_slots(&bytes)?;
    let encoded = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(TestEntity::new(1, "Self", "synthetic/root.xml").record())
        .build();
    let forge = DataForge::open(&encoded)?;
    let recorder = Arc::new(Recorder::new(&forge, Stop::Never));
    let resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), recorder.clone());
    let document = document(&Files::new(bytes), &resolver)?;
    let class = loadout_xml::parse(&document, resolver.observation.as_deref(), |root, scope| {
        scope.class_name(root, "itemName")
    })?
    .ok_or("the actual original winning class")?;
    assert_eq!(
        class.value(),
        "Self",
        "the exact original winner survives the source callback"
    );
    let log = recorder.log.lock().map_err(|_| "source log")?;
    assert_eq!(
        log.xml.len(),
        1,
        "only the actual accepted slot is observed"
    );
    let original = log.xml.first().ok_or("the original XML getter")?;
    assert_eq!(
        (original.node_index, original.attribute_index),
        (1, 1),
        "node and decoder-attribute ordinal uniquely identify the real winner"
    );
    assert_eq!(
        class.selection(),
        Some(&original.selection),
        "the ordinal is bound to the same actual getter token"
    );
    drop(log);
    Ok(())
}
