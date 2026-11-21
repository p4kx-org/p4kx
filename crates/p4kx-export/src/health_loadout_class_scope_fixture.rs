// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_extraction::{
    Archive, DataForge, Guid, InputScope, PhysicalSourceRef, Record, SourceInput, SourceUses,
};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry, TestLoadout};

use super::TestResult;

const ROOT: &str = "synthetic/graph-root.xml";
const LEFT: &str = "synthetic/graph-left.xml";
const RIGHT: &str = "synthetic/graph-right.xml";
const SHARED: &str = "synthetic/graph-shared.xml";
const ROOT_XML: &[u8] = br#"<Loadout><Loadouts><Loadout loadout="synthetic/graph-left.xml"/><Loadout loadout="synthetic/graph-right.xml"/></Loadouts></Loadout>"#;
const LEFT_XML: &[u8] = br#"<Loadout><Loadouts><Loadout loadout="synthetic/graph-shared.xml"/></Loadouts><Items><Item portName="selected" itemName="Other"/></Items></Loadout>"#;
const RIGHT_XML: &[u8] =
    br#"<Loadout><Loadouts><Loadout loadout="synthetic/graph-shared.xml"/></Loadouts></Loadout>"#;
const SHARED_XML: &[u8] =
    br#"<Loadout><Items><Item portName="selected" itemName="Self"/></Items></Loadout>"#;
const DOCUMENTS: [(&str, &[u8]); 4] = [
    (ROOT, ROOT_XML),
    (LEFT, LEFT_XML),
    (RIGHT, RIGHT_XML),
    (SHARED, SHARED_XML),
];

pub(super) fn database() -> Vec<u8> {
    let mut database = p4kx_testkit::entity_schema(DataForgeBuilder::new());
    for index in 0..3 {
        database = database.record(
            TestEntity::new(1, "Self", format!("synthetic/physical-{index}.xml"))
                .loadout(TestLoadout::Xml(ROOT.to_owned()))
                .record(),
        );
    }
    database
        .record(TestEntity::new(2, "Other", "synthetic/other.xml").record())
        .build()
}

pub(super) fn entries() -> Vec<TestEntry> {
    DOCUMENTS
        .into_iter()
        .map(|(path, bytes)| TestEntry::stored(format!("Data/{path}"), bytes))
        .collect()
}

pub(super) fn original(archive: &Archive, forge: &DataForge<'_>) -> TestResult<(Guid, u64, u64)> {
    assert_eq!(
        forge.records().count(),
        4,
        "three actual same-class callers and one losing target"
    );
    let first = forge.record(0).ok_or("first original Self definition")?;
    check_callers(forge, &first)?;
    let mut root_attributes = Vec::new();
    for (path, wanted) in DOCUMENTS {
        let entry = archive
            .entries()
            .iter()
            .find(|entry| entry.path() == format!("Data/{path}"))
            .ok_or("original graph source entry")?;
        let mut bytes = Vec::new();
        archive.open_entry(entry)?.read_to_end(&mut bytes)?;
        assert_eq!(bytes, wanted, "every actual selected archive source byte");
        let document = roxmltree::Document::parse(std::str::from_utf8(&bytes)?)?;
        let attributes: Vec<_> = document
            .descendants()
            .filter_map(|node| {
                node.attribute("loadout")
                    .map(|value| (u64::from(node.id().get()), value))
            })
            .collect();
        match path {
            ROOT => {
                assert_eq!(
                    attributes,
                    [(3, LEFT), (4, RIGHT)],
                    "genuine ordered separate root Include slots"
                );
                root_attributes.extend(attributes.iter().map(|(node, _)| *node));
            }
            LEFT | RIGHT => assert_eq!(
                attributes,
                [(3, SHARED)],
                "both actual branches select the same cached child"
            ),
            _ => assert!(attributes.is_empty(), "the genuine shared child is a leaf"),
        }
        check_items(&document, path);
    }
    Ok((
        first.guid(),
        *root_attributes.first().ok_or("original left Include")?,
        *root_attributes.get(1).ok_or("original right Include")?,
    ))
}

fn check_callers(forge: &DataForge<'_>, first: &Record<'_>) -> TestResult {
    for index in 0..3 {
        let record = forge.record(index).ok_or("physical original caller")?;
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record) && tree.error_count() == 0,
            "genuine readable caller"
        );
        assert_eq!(
            record.guid(),
            first.guid(),
            "all three roots actually cycle against this GUID"
        );
        assert_eq!(
            record.name()?,
            first.name()?,
            "the nominal class name is the same"
        );
        assert_eq!(
            record.index(),
            index,
            "the original roots have distinct physical positions"
        );
    }
    Ok(())
}

fn check_items(document: &roxmltree::Document<'_>, path: &str) {
    let items: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("Item"))
        .map(|node| (node.attribute("portName"), node.attribute("itemName")))
        .collect();
    match path {
        LEFT => assert_eq!(
            items,
            [(Some("selected"), Some("Other"))],
            "Left shadows the shared class on the same port"
        ),
        SHARED => assert_eq!(
            items,
            [(Some("selected"), Some("Self"))],
            "the original shared class target"
        ),
        _ => assert!(
            items.is_empty(),
            "no other item can supply the actual winner"
        ),
    }
}

pub(super) struct IncludeSources {
    pub(super) root: PhysicalSourceRef,
    pub(super) right: PhysicalSourceRef,
}

pub(super) fn include_sources(archive: &Archive, uses: &SourceUses) -> TestResult<IncludeSources> {
    let source = |path| -> TestResult<PhysicalSourceRef> {
        let entry = archive
            .entries()
            .iter()
            .find(|entry| entry.path() == format!("Data/{path}"))
            .ok_or("the actual original Include source Entry")?;
        let origin = archive
            .entry_origin(entry)
            .ok_or("the actual original Entry admission")?;
        let scoped = uses.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
        Ok(scoped.source_ref().clone())
    };
    let root = source(ROOT)?;
    let right = source(RIGHT)?;
    assert_ne!(
        root, right,
        "the two original documents are different physical sources"
    );
    Ok(IncludeSources { root, right })
}
