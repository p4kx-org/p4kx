// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::sync::Arc;

use p4kx_extraction::{
    Archive, CancelToken, EntryIssue, OpenOptions, SourceInventory, SourceLocation, SourceUseRole,
};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestContainer, TestElement, TestEntry,
};
use serde_json::{Value, json};

use crate::catalog_read::original::{EntryRead, EntryXml, with_entry_capture};
use crate::catalog_rows::spool::RowSpoolError;
use crate::entry_source::EntrySource;
use crate::loot_walk;
use crate::starmap_index::Index;
use crate::starmap_walk::Opened;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const ROOT: &str = "Data/ObjectContainers/PU/synthetic/root.socpak";
const MISSING: &str = "Data/ObjectContainers/PU/synthetic/missing.socpak";
const KNOWN: &str = "Data/ObjectContainers/PU/synthetic/known.socpak";
const AMBIGUOUS: &str = "Data/ObjectContainers/PU/synthetic/ambiguous.socpak";

fn open(xmls: &[&str], targets: &[&str]) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let mut container = TestContainer::new();
    for xml in xmls {
        container = container.entry(TestEntry::stored("entdata/repeated.entxml", *xml));
    }
    let mut archive = ArchiveBuilder::new().entry(TestEntry::stored(ROOT, container.build()));
    for &target in targets {
        archive = archive.entry(TestEntry::stored(target, TestContainer::new().build()));
    }
    archive.write_to(&path)?;
    Ok((directory, Archive::open(&path)?))
}

fn roots() -> Vec<Opened> {
    (0..2)
        .map(|_| Opened {
            system_entry: 0,
            system: Arc::from("synthetic"),
            entry: 0,
            under: None,
            parent: crate::starmap_walk::OpenedParent::Unobserved,
        })
        .collect()
}

fn read(archive: &Archive) -> TestResult<(Value, SourceInventory)> {
    with_entry_capture(archive, |capture| {
        let index = Index::new(archive);
        let read = loot_walk::explore(archive, &index, &roots(), None, capture)?;
        let table =
            serde_json::to_value(read.original_documents).map_err(crate::CatalogError::Json)?;
        Ok((table, capture.source_uses().freeze()?))
    })
    .map_err(Into::into)
}

#[test]
fn original_links_keep_physical_documents_attribute_slots_and_separate_entity_owners() -> TestResult
{
    let xml = format!(
        r#"<Root><Entity CryGUID="00000000-0000-0000-0000-000000000001" pos=" 1, 2, 3 ">
        <a objectContainer="{MISSING}"/><b objectContainer="{MISSING}"/>
        <c multiConfigRef="00000000-0000-0000-0000-000000000000"/></Entity>
        <Entity CryGUID="00000000-0000-0000-0000-000000000001"><d multiConfigRef="invalid"/>
        <e objectContainer="{KNOWN}"/></Entity><f multiConfigRef=""/>
        <g objectContainer="{AMBIGUOUS}"/><h objectContainer="C:/authoring/unknown.socpak"/>
        <i objectContainer=""/></Root>"#
    );
    let (_directory, archive) = open(&[&xml, &xml], &[KNOWN, AMBIGUOUS, AMBIGUOUS])?;
    let (table, inventory) = read(&archive)?;
    let documents = table["rows"].as_array().ok_or("original document rows")?;
    assert_eq!(
        documents.len(),
        2,
        "repeated placement roots do not multiply physical source documents"
    );
    assert_ne!(
        documents[0]["document"]["value"], documents[1]["document"]["value"],
        "same path and bytes remain distinct physical Entries"
    );
    assert_eq!(
        documents[0]["containingContainer"],
        documents[1]["containingContainer"]
    );
    verify_sites(&documents[0], &inventory)?;
    Ok(())
}

fn verify_sites(document: &Value, inventory: &SourceInventory) -> TestResult {
    let links = document["links"]["value"]
        .as_array()
        .ok_or("actual link sites")?;
    assert_eq!(
        links.len(),
        6,
        "each original attribute, including an empty one, remains"
    );
    let states: Vec<_> = links
        .iter()
        .map(|row| &row["targetStatus"]["value"])
        .collect();
    assert_eq!(
        states,
        [
            &json!("notFound"),
            &json!("notFound"),
            &json!("resolved"),
            &json!("ambiguous"),
            &json!("unsupportedSelector"),
            &json!("unsupportedSelector")
        ]
    );
    assert_ne!(
        links[0]["nodeIndex"], links[1]["nodeIndex"],
        "equal path text does not collapse its authored sites"
    );
    assert_ne!(
        links[0]["declaredPath"]["sourceUsesRefs"],
        links[1]["declaredPath"]["sourceUsesRefs"]
    );
    assert_eq!(
        links[3]["targetCandidates"]["value"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    verify_configs_and_owners(document)?;
    verify_lookup(&links[0], inventory)?;
    Ok(())
}

fn verify_configs_and_owners(document: &Value) -> TestResult {
    let configs = document["configReferences"]["value"]
        .as_array()
        .ok_or("original config attributes")?;
    assert_eq!(
        configs.len(),
        3,
        "config occurrences appear once per physical document"
    );
    assert_eq!(
        configs[0]["guid"]["state"], "present",
        "authored zero GUID remains present"
    );
    assert_eq!(
        configs[0]["guid"]["value"],
        "00000000-0000-0000-0000-000000000000"
    );
    assert_eq!(configs[1]["guid"]["state"], "invalid");
    assert_eq!(
        raw_attribute(document, &configs[2])?["value"],
        "",
        "original empty text is not filtered"
    );
    let nodes = document["nodes"]["value"]
        .as_array()
        .ok_or("normalized original nodes")?;
    let entities: Vec<_> = nodes
        .iter()
        .filter(|node| node["name"]["value"] == "Entity")
        .collect();
    assert_eq!(
        entities.len(),
        2,
        "equal Entity GUIDs do not merge XML owners"
    );
    assert_ne!(
        entities[0]["nodeIndex"]["value"],
        entities[1]["nodeIndex"]["value"]
    );
    assert_eq!(
        entities[0]["parentNodeIndex"]["state"], "present",
        "the original non-Entity document root stays the actual parent"
    );
    Ok(())
}

fn raw_attribute<'row>(document: &'row Value, reference: &Value) -> TestResult<&'row Value> {
    let ordinal = usize::try_from(
        reference["attributeOrdinal"]["value"]
            .as_u64()
            .ok_or("attribute ordinal")?,
    )?;
    let node = document["nodes"]["value"]
        .as_array()
        .ok_or("original nodes")?
        .iter()
        .find(|node| node["nodeIndex"]["value"] == reference["nodeIndex"]["value"])
        .ok_or("the referenced original node")?;
    node["attributes"]
        .as_array()
        .and_then(|attributes| attributes.get(ordinal))
        .and_then(|attribute| attribute.get("value"))
        .ok_or_else(|| "the referenced original attribute".into())
}

fn verify_lookup(link: &Value, inventory: &SourceInventory) -> TestResult {
    let refs = link["targetStatus"]["sourceUsesRefs"]
        .as_array()
        .ok_or("lookup provenance")?;
    assert_eq!(
        link["targetStatus"]["sourceUsesRefs"],
        link["targetCandidates"]["sourceUsesRefs"]
    );
    assert!(
        inventory.uses().iter().any(|used| matches!(
            used.location(),
            SourceLocation::ArchiveDirectoryPaths { .. }
        ) && refs.contains(&json!(used.id().as_str()))),
        "negative membership cites the original root directory survey"
    );
    let raw = link["declaredPath"]["sourceUsesRefs"]
        .as_array()
        .ok_or("original selector proof")?;
    assert!(inventory.uses().iter().any(|used| used.role() == SourceUseRole::Payload && raw.contains(&json!(used.id().as_str())) && matches!(used.location(), SourceLocation::Subresource { namespace, indices, field } if namespace == "xml" && indices.len() == 2 && field == "@objectContainer")), "raw value keeps its actual attribute ordinal and Payload role");
    Ok(())
}

#[test]
fn recovered_root_directory_keeps_candidate_evidence_without_certifying_absence() -> TestResult {
    let xml = format!(
        r#"<Entity><a objectContainer="{MISSING}"/><b objectContainer="{AMBIGUOUS}"/></Entity>"#
    );
    let (directory, archive) = open(&[&xml], &[AMBIGUOUS, AMBIGUOUS])?;
    drop(archive);
    let path = directory.path().join("Data.p4k");
    let mut bytes = fs::read(&path)?;
    let end = bytes
        .windows(4)
        .rposition(|part| part == b"PK\x05\x06")
        .ok_or("actual end record")?;
    let zip64_end = bytes
        .windows(4)
        .rposition(|part| part == b"PK\x06\x06")
        .ok_or("actual outer ZIP64 end record")?;
    bytes
        .get_mut(zip64_end..end)
        .ok_or("outer ZIP64 end and locator range")?
        .fill(0);
    fs::write(&path, bytes)?;
    assert!(
        Archive::open(&path).is_err(),
        "the outer directory cannot open through an intact inner end record"
    );
    let archive = Archive::open_with(&path, OpenOptions::new().recover(true))?;
    assert!(
        archive
            .issues()
            .iter()
            .any(|(_, issue)| *issue == EntryIssue::Recovered),
        "the original outer directory was actually recovered"
    );
    assert_eq!(
        archive.entries().len(),
        3,
        "recovery retains the root and both distinct duplicate targets"
    );
    assert!(crate::export_archive_health::inventory(&archive).is_some());
    let (table, _) = read(&archive)?;
    let links = &table["rows"][0]["links"]["value"];
    assert_eq!(links[0]["targetStatus"]["value"], "incompleteDirectory");
    assert_eq!(links[0]["targetCandidates"]["sourceSetComplete"], false);
    assert_eq!(
        links[1]["targetStatus"]["value"], "ambiguous",
        "actual duplicates already disprove uniqueness"
    );
    assert_eq!(links[1]["targetCandidates"]["sourceSetComplete"], false);
    Ok(())
}

#[test]
fn empty_and_unreadable_reached_scopes_have_distinct_availability() -> TestResult {
    for (xml, expected) in [("<Entity/>", "absent"), ("broken", "unavailable")] {
        let (_directory, archive) = open(&[xml], &[])?;
        let (table, _) = read(&archive)?;
        assert_eq!(table["availability"], expected);
        assert_eq!(table["rows"], json!([]));
    }
    Ok(())
}

#[test]
fn original_document_spool_replay_latches_cancellation() -> TestResult {
    let xml = format!(r#"<Entity objectContainer="{MISSING}"/>"#);
    let (_directory, archive) = open(&[&xml], &[])?;
    let cancel = CancelToken::new();
    with_entry_capture(&archive, |capture| {
        let read = loot_walk::explore(
            &archive,
            &Index::new(&archive),
            &roots(),
            Some(&cancel),
            capture,
        )?;
        cancel.cancel();
        assert!(
            serde_json::to_value(&read.original_documents).is_err(),
            "cancelled staged originals cannot become a complete table"
        );
        assert!(matches!(
            read.original_documents.take_failure(),
            Some(RowSpoolError::Cancelled)
        ));
        Ok(())
    })?;
    Ok(())
}

#[test]
fn repeated_cryxml_attribute_names_keep_each_original_ordinal() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let xml = TestElement::new("Entity")
        .attribute("objectContainer", MISSING)
        .attribute("objectContainer", KNOWN)
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let container = TestContainer::new().entry(TestEntry::stored("entdata/repeated.entxml", xml));
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ROOT, container.build()))
        .entry(TestEntry::stored(KNOWN, TestContainer::new().build()))
        .write_to(&path)?;
    let (table, _) = read(&Archive::open(&path)?)?;
    let links = &table["rows"][0]["links"]["value"];
    assert_eq!(links.as_array().map(Vec::len), Some(2));
    assert_eq!(
        links[0]["nodeIndex"]["value"], links[1]["nodeIndex"]["value"],
        "two attributes belong to one actual source node"
    );
    assert_eq!(links[0]["attributeOrdinal"]["value"], 0);
    assert_eq!(links[1]["attributeOrdinal"]["value"], 1);
    assert_eq!(links[0]["declaredPath"]["value"], MISSING);
    assert_eq!(links[1]["declaredPath"]["value"], KNOWN);
    assert_ne!(
        links[0]["declaredPath"]["sourceUsesRefs"],
        links[1]["declaredPath"]["sourceUsesRefs"]
    );
    assert_eq!(
        table["rows"][0]["nodes"]["value"].as_array().map(Vec::len),
        Some(1),
        "the original node and its attributes are normalized once"
    );
    Ok(())
}

#[test]
fn a_bounded_read_cannot_supply_a_complete_document_survey() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("source.xml", "<Entity/>"))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    with_entry_capture(&archive, |capture| {
        let entry = archive.entries().first().expect("one original Entry");
        let source = EntrySource::outer(capture, &archive, entry)?;
        let read = EntryRead::read(source.receipt().clone(), &archive, entry, 1024)?
            .expect("small valid original XML bytes");
        let xml = EntryXml::parse(&read, capture)?.expect("valid original XML");
        assert!(
            xml.document_source().is_err(),
            "successful parse does not reconstitute missing EOF-read authority"
        );
        Ok(())
    })?;
    Ok(())
}
