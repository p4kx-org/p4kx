// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::error::Error;
use std::path::Path;

use p4kx_extraction::{Archive, SourceLocation, SourceUseRole};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestChunkFile, TestContainer, TestElement,
    TestEntry,
};

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, with_entry_capture};
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::SourceDisposition;
use crate::shop_container_scan::{self, Scanned};
use crate::shop_containers::Component;
use crate::shop_inventory_observations::RejectedInventoryPath;
use crate::{CatalogError, Kind};

type TestResult = Result<(), Box<dyn Error>>;

const FIRST: &str = "a0000000-0000-4000-8000-000000000001";
const SECOND: &str = "a0000000-0000-4000-8000-000000000002";
const INVENTORY: &str = "Scripts/ShopInventories/fixture.json";

#[test]
fn a_shared_cryxml_value_uses_each_authored_attribute_and_exact_entity() -> TestResult {
    let original = TestElement::new("Entities")
        .child(
            entity(FIRST).child(
                TestElement::new("PropertiesDataCore").child(
                    TestElement::new("EntityComponentShop")
                        .attribute("irrelevant", "stop lexical inference")
                        .attribute("offlineInventoryJSON", INVENTORY)
                        .child(
                            TestElement::new("SShopComponentParams")
                                .attribute("offlineInventoryNested", INVENTORY),
                        ),
                ),
            ),
        )
        .child(entity(SECOND).child(TestElement::new("Components").child(
            TestElement::new("EntityComponentShop").attribute("offlineInventory", INVENTORY),
        )));
    let bytes = original.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let soc = TestChunkFile::crch().chunk(0x0004, 3, bytes).bytes();
    let temporary = tempfile::tempdir()?;
    let archive = archive(temporary.path(), metadata(&[FIRST, SECOND]), soc)?;
    with_entry_capture(&archive, |capture| {
        let result =
            shop_container_scan::shop_entities(&archive, &archive.entries()[0], None, capture)?;
        assert_audited_entities(result, capture)
    })?;
    Ok(())
}

fn assert_audited_entities(
    result: Scanned,
    capture: &EntryCapture<'_>,
) -> Result<(), CatalogError> {
    assert!(
        !result.unattributed_declarations,
        "both original owners have one metadata declaration"
    );
    assert!(
        result.ledger.incomplete_inventories.is_empty(),
        "all entity chunks decoded"
    );
    assert!(
        result.ledger.entries.iter().any(|source| {
            source.operation == EntryOperation::ShopSoc
                && source.disposition == SourceDisposition::Projected
        }),
        "the normal Shop SOC source ledger accepts the structural projection"
    );
    let Some((entities, 0)) = result.entities else {
        panic!("expected all inventory paths to have an original owner");
    };
    assert_eq!(entities.len(), 2, "each placed Entity is exported");
    let mut slots = HashSet::new();
    for ((guid, component), (expected, count)) in entities.iter().zip([(FIRST, 2), (SECOND, 1)]) {
        assert_eq!(guid, expected, "metadata order and exact owner survive");
        let Component::File(stem) = component else {
            panic!("expected inventory file for {guid}");
        };
        assert_eq!(stem.as_str(), "fixture", "existing inventory normalization");
        assert_eq!(
            stem.declarations().len(),
            count,
            "physical slots are not collapsed"
        );
        for declaration in stem.declarations() {
            assert_attribute(declaration, capture, &mut slots)?;
        }
    }
    Ok(())
}

fn assert_attribute(
    declaration: &AuthoredEntryPath,
    capture: &EntryCapture<'_>,
    slots: &mut HashSet<Vec<u64>>,
) -> Result<(), CatalogError> {
    let original = declaration.retain_in(capture)?;
    let (uses, complete) = original.contributions();
    assert!(
        complete,
        "field, original owner and metadata evidence are available"
    );
    let payloads = uses
        .iter()
        .filter(|used| used.role() == SourceUseRole::Payload)
        .collect::<Vec<_>>();
    let [payload] = payloads.as_slice() else {
        panic!("one original field payload source");
    };
    let SourceLocation::Subresource {
        namespace,
        indices,
        field,
    } = payload.description().location()
    else {
        panic!("an exact original SOC attribute coordinate");
    };
    assert_eq!(
        namespace, "socCryXml",
        "structural evidence replaces lexical offsets"
    );
    assert_eq!(
        indices.len(),
        3,
        "chunk, node and original attribute indices"
    );
    assert!(
        field.starts_with("offlineInventory"),
        "the original field name remains attached"
    );
    assert!(
        slots.insert(indices.to_owned()),
        "each attribute is a distinct physical source"
    );
    assert!(
        uses.iter()
            .any(|used| used.role() == SourceUseRole::Selector
                && matches!(used.description().location(),
                    SourceLocation::Subresource { field, .. } if field == "EntityCryGUID"
                )),
        "the original SOC owner selects the value"
    );
    Ok(())
}

#[test]
fn structural_invalid_values_and_unreadable_chunks_keep_the_source_unavailable() -> TestResult {
    let original =
        TestElement::new("Entities")
            .child(
                entity(FIRST).child(
                    TestElement::new("EntityComponentShop")
                        .attribute("offlineInventoryJSON", INVENTORY)
                        .attribute("offlineInventoryXMLName", "unsupported.csv"),
                ),
            )
            .child(entity(SECOND).child(
                TestElement::new("EntityComponentShop").attribute("offlineInventoryJSON", ""),
            ));
    let soc = TestChunkFile::crch()
        .chunk(0x0004, 3, b"<broken".to_vec())
        .chunk(0x0004, 3, original.to_xml().into_bytes())
        .bytes();
    let temporary = tempfile::tempdir()?;
    let archive = archive(temporary.path(), metadata(&[FIRST, SECOND]), soc)?;
    let result = with_entry_capture(&archive, |capture| {
        shop_container_scan::shop_entities(&archive, &archive.entries()[0], None, capture)
    })?;
    assert!(
        result.ledger.incomplete_inventories.contains(&Kind::Shops),
        "a failed entity chunk cannot be counted as complete"
    );
    assert!(
        result.ledger.entries.iter().any(|source| {
            source.operation == EntryOperation::ShopSoc
                && source.disposition == SourceDisposition::ProjectionFailed
                && source.diagnosis.code == Some(ExportDiagnosticCode::MalformedSource)
        }),
        "actual source failure remains explicit"
    );
    let rejected = &result.observations[FIRST].rejected;
    assert!(
        matches!(rejected.as_slice(), [RejectedInventoryPath::Text(path)]
        if path.as_str() == "unsupported.csv"),
        "known invalid original text is retained"
    );
    let no_file = &result.observations[SECOND].no_file;
    assert!(
        no_file.iter().any(|path| path.as_str().is_empty()),
        "explicit empty attribute retained"
    );
    let Some((entities, _)) = result.entities else {
        panic!("lossless sibling chunk should still be projected");
    };
    assert!(
        matches!(
            &entities[..],
            [(_, Component::File(_)), (_, Component::NoFile)]
        ),
        "valid and empty owner declarations survive a separate failed chunk"
    );
    Ok(())
}

#[test]
fn duplicate_metadata_cannot_assign_a_structured_inventory_to_an_owner() -> TestResult {
    let original =
        TestElement::new("Entities").child(entity(FIRST).child(
            TestElement::new("EntityComponentShop").attribute("offlineInventory", INVENTORY),
        ));
    let soc = TestChunkFile::crch()
        .chunk(0x0004, 3, original.to_xml().into_bytes())
        .bytes();
    let temporary = tempfile::tempdir()?;
    let archive = archive(temporary.path(), metadata(&[FIRST, FIRST]), soc)?;
    let result = with_entry_capture(&archive, |capture| {
        shop_container_scan::shop_entities(&archive, &archive.entries()[0], None, capture)
    })?;
    let Some((entities, rejected)) = result.entities else {
        panic!("the metadata is readable even though ownership is ambiguous");
    };
    assert_eq!(
        rejected, 1,
        "the exact field could not be assigned uniquely"
    );
    assert!(
        entities
            .iter()
            .all(|(_, component)| matches!(component, Component::NotFound)),
        "an ambiguous metadata join cannot publish a selected file"
    );
    assert!(
        result.unattributed_declarations,
        "the declaration inventory remains unresolved"
    );
    Ok(())
}

fn entity(guid: &str) -> TestElement {
    TestElement::new("Entity").attribute("EntityCryGUID", guid)
}

fn metadata(guids: &[&str]) -> String {
    let mut root = TestElement::new("Entities");
    for guid in guids {
        root = root.child(TestElement::new("Entity").attribute("CryGUID", *guid));
    }
    root.to_xml()
}

fn archive(directory: &Path, metadata: String, soc: Vec<u8>) -> Result<Archive, Box<dyn Error>> {
    let container = TestContainer::new()
        .entry(TestEntry::stored(
            "metadata/entitycomponentshop.xml",
            metadata,
        ))
        .entry(TestEntry::stored("fixture.soc", soc));
    let path = directory.join("fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/ObjectContainers/PU/Shops/fixture.socpak",
            container.build(),
        ))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}
