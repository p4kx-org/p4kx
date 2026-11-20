// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_export::build_catalog_from;
use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::loot::{
    LootDocumentQuery, LootDocumentReachability, LootDocumentSelection, read_loot_documents,
};
use p4kx_extraction::{
    Archive, EntryIssue, InputScope, OpenOptions, PhysicalSourceKind, SourceEncryptionState,
    SourceLocation, SourceUseRole,
};
use p4kx_testkit::{TestContainer, TestEntry};
use serde_json::{Value, json};

use super::{
    ENTITY_DOCUMENT, ROOT, TARGET, source, source_documents, source_documents_and_targets,
};
use crate::health_origin_assertions as proof;
use crate::health_origin_entry_source as entries;
use crate::health_origin_fixture::{self as fixture, Fixture, TestResult};

#[test]
fn sdk_selects_physical_loot_document_aliases_from_the_same_catalog_scope() -> TestResult {
    let source = source_documents([
        TestEntry::stored(
            ENTITY_DOCUMENT,
            format!(r#"<Entity objectContainer="{TARGET}"/>"#),
        ),
        TestEntry::stored(
            ENTITY_DOCUMENT,
            format!(r#"<Entity objectContainer="{TARGET}"/>"#),
        ),
    ])?;
    let published = build_catalog_from(&source.archive, &source.entry, &fixture::options())?;
    let catalog: Value = serde_json::from_slice(published.json())?;
    let expected = proof::rows(&catalog, "/declarations/lootEntityContainerDocuments/rows")?;
    let all = read_loot_documents(
        &source.archive,
        LootDocumentQuery::new(LootDocumentSelection::All),
        None,
    )?;
    assert!(matches!(all.availability, DatasetAvailability::Available));
    assert_eq!(
        serde_json::to_value(&all.rows)?,
        Value::Array(expected.to_vec()),
        "whole selected document table matches the Catalog"
    );
    let [first, alias] = all.rows.as_slice() else {
        return Err("both original physical document aliases".into());
    };
    let outer = first
        .containing_container()
        .value()
        .ok_or("original physical parent")?
        .entry_index();
    let inner = alias
        .document()
        .value()
        .ok_or("original physical alias")?
        .entry_index();
    assert_ne!(
        first.document().value(),
        alias.document().value(),
        "identical entry names retain different physical identities"
    );
    verify_exact_alias(&source.archive, outer, inner, &expected[1])
}

fn verify_exact_alias(archive: &Archive, outer: u32, inner: u32, expected: &Value) -> TestResult {
    let selection = LootDocumentSelection::Document {
        container_entry_index: outer,
        document_entry_index: inner,
    };
    let selected = read_loot_documents(archive, LootDocumentQuery::new(selection), None)?;
    assert_eq!(
        selected.coverage.reachability,
        LootDocumentReachability::Reached,
        "the selected alias retains its observed route"
    );
    assert!(
        selected.coverage.documents_complete,
        "the selected physical alias is fully observed"
    );
    assert_eq!(
        selected.coverage.visited_loot_containers, 1,
        "an admitted seed needs no other Loot bodies"
    );
    assert_eq!(
        serde_json::to_value(&selected.rows)?,
        json!([expected]),
        "the physical owner and its original source uses survive exact selection"
    );
    let selected = selected.rows.first().ok_or("one exact document")?;
    let link = selected
        .links()
        .value()
        .and_then(|links| links.first())
        .ok_or("original selector")?;
    assert_eq!(
        link.declared_path().origin().contains_encrypted_source(),
        Some(true),
        "the original encrypted entity member remains attributable"
    );
    Ok(())
}

#[test]
fn sdk_keeps_unreachable_physical_owners_distinct_from_unknown_reachability() -> TestResult {
    const UNREACHED: &str = "Data/ObjectContainers/PU/synthetic/unreached.socpak";
    for unknown in [false, true] {
        let source = source_documents_and_targets(
            unknown.then(|| TestEntry::stored(ENTITY_DOCUMENT, b"broken XML")),
            [TestEntry::stored(UNREACHED, TestContainer::new().build())],
        )?;
        let candidate = source
            .archive
            .entries()
            .iter()
            .find(|entry| entry.path() == UNREACHED)
            .ok_or("unreached fixture owner")?;
        let selection = LootDocumentSelection::Container {
            container_entry_index: u32::try_from(candidate.index())?,
        };
        let queried =
            read_loot_documents(&source.archive, LootDocumentQuery::new(selection), None)?;
        assert!(
            queried.rows.is_empty(),
            "mere physical presence cannot admit a document"
        );
        if unknown {
            assert_eq!(
                queried.coverage.reachability,
                LootDocumentReachability::Unavailable
            );
            assert!(matches!(
                queried.availability,
                DatasetAvailability::Unavailable
            ));
        } else {
            assert_eq!(
                queried.coverage.reachability,
                LootDocumentReachability::NotReached
            );
            assert!(matches!(queried.availability, DatasetAvailability::Absent));
        }
    }
    Ok(())
}

#[test]
fn sdk_missing_outer_entry_needs_an_intact_directory() -> TestResult {
    let source = entries::with_starmap(vec![TestEntry::stored(
        "Data/ObjectContainers/PU/synthetic/recovered.socpak",
        TestContainer::new().build(),
    )])?;
    let count = u32::try_from(source.archive.entries().len())?;
    let intact = read_loot_documents(
        &source.archive,
        LootDocumentQuery::new(LootDocumentSelection::Container {
            container_entry_index: count,
        }),
        None,
    )?;
    assert!(matches!(intact.availability, DatasetAvailability::Absent));
    assert!(intact.coverage.directory_complete && intact.coverage.documents_complete);
    assert_eq!(
        intact.coverage.reachability,
        LootDocumentReachability::NotReached
    );
    assert_ordinal_evidence(&intact.source_inventory);
    verify_recovered_missing_outer(source, count)
}

fn verify_recovered_missing_outer(source: Fixture, count: u32) -> TestResult {
    let Fixture {
        folder,
        archive,
        entry,
    } = source;
    drop((archive, entry));
    let path = folder.path().join("Data.p4k");
    let mut bytes = fs::read(&path)?;
    crate::export_health_recovery::remove_directory_record_and_end(&mut bytes)?;
    fs::write(&path, bytes)?;
    let archive = Archive::open_with(&path, OpenOptions::new().recover(true))?;
    let missing = count.checked_sub(1).ok_or("original removed ordinal")?;
    assert_eq!(
        archive.entries().len(),
        usize::try_from(missing)?,
        "the recovered inventory must omit the removed directory slot"
    );
    assert!(
        archive
            .issues()
            .iter()
            .any(|(_, issue)| matches!(issue, EntryIssue::Recovered)),
        "the source actually lost an original directory Entry"
    );
    let recovered = read_loot_documents(
        &archive,
        LootDocumentQuery::new(LootDocumentSelection::Container {
            container_entry_index: missing,
        }),
        None,
    )?;
    assert!(recovered.rows.is_empty(), "the lost entry supplies no row");
    assert!(
        matches!(recovered.availability, DatasetAvailability::Unavailable),
        "a recovered directory cannot certify absence"
    );
    assert!(
        !recovered.coverage.directory_complete && !recovered.coverage.documents_complete,
        "a recovered entry count cannot certify directory or document completeness"
    );
    assert_eq!(
        recovered.coverage.reachability,
        LootDocumentReachability::Unavailable,
        "the reader cannot exclude the original ordinal"
    );
    assert_ordinal_evidence(&recovered.source_inventory);
    Ok(())
}

fn assert_ordinal_evidence(inventory: &p4kx_extraction::SourceInventory) {
    assert!(
        inventory.iter_uses().any(|used| matches!(
            used.location(),
            SourceLocation::Metadata { field } if field == "entryCount"
        )),
        "the admitted directory count is the observed ordinal evidence"
    );
    assert!(
        !inventory.iter_uses().any(|used| matches!(
            used.location(),
            SourceLocation::ArchiveDirectoryPaths { .. }
        )),
        "no path survey is attributed to an ordinal-only lookup"
    );
}

#[test]
fn sdk_nested_ordinal_absence_keeps_the_actual_outer_payload_count() -> TestResult {
    let source = source(None)?;
    let outer = source
        .archive
        .entries()
        .iter()
        .find(|entry| entry.path() == ROOT)
        .ok_or("prepared outer container")?;
    let container = source.archive.open_container(outer)?;
    let count = u32::try_from(container.entries().len())?;
    let selected = read_loot_documents(
        &source.archive,
        LootDocumentQuery::new(LootDocumentSelection::Document {
            container_entry_index: u32::try_from(outer.index())?,
            document_entry_index: count,
        }),
        None,
    )?;
    assert_eq!(
        selected.coverage.reachability,
        LootDocumentReachability::Reached
    );
    assert!(selected.coverage.documents_complete);
    assert!(matches!(selected.availability, DatasetAvailability::Absent));
    let sources = &selected.source_inventory;
    let outer_source = sources
        .iter_sources()
        .find(|source| {
            matches!(
                source.kind(), PhysicalSourceKind::ArchiveEntry(entry)
                    if source.ancestors().is_empty() && entry.entry_index() == outer.index()
            )
        })
        .ok_or("actual outer directory owner")?;
    assert!(sources.iter_uses().any(|used| {
        used.source() == outer_source.id()
            && used.scope() == InputScope::Payload
            && used.role() == SourceUseRole::Selector
            && used.effective_encryption() == SourceEncryptionState::Encrypted
            && matches!(used.location(), SourceLocation::Subresource { namespace, indices, field }
                if namespace == "socArchiveDirectory" && indices.as_slice() == [0, u64::from(count)]
                    && field == "entryCount")
    }), "the admitted count belongs to the selected encrypted container payload");
    Ok(())
}

#[test]
fn sdk_ineligible_nested_entry_keeps_its_original_path_selector() -> TestResult {
    const UNRELATED: &str = "unrelated.bin";
    let source = source_documents([TestEntry::stored(UNRELATED, b"fixture")])?;
    let outer = source
        .archive
        .entries()
        .iter()
        .find(|entry| entry.path() == ROOT)
        .ok_or("prepared outer container")?;
    let container = source.archive.open_container(outer)?;
    let inner = container
        .entries()
        .iter()
        .find(|entry| entry.path() == UNRELATED)
        .ok_or("exact member that does not qualify as EntXML")?;
    let selected = read_loot_documents(
        &source.archive,
        LootDocumentQuery::new(LootDocumentSelection::Document {
            container_entry_index: u32::try_from(outer.index())?,
            document_entry_index: u32::try_from(inner.index())?,
        }),
        None,
    )?;
    assert!(matches!(selected.availability, DatasetAvailability::Absent));
    assert!(selected.coverage.documents_complete);
    assert_eq!(
        selected.coverage.reachability,
        LootDocumentReachability::Reached
    );
    let sources = &selected.source_inventory;
    let inner_source = sources.iter_sources().find(|source| matches!(
        source.kind(), PhysicalSourceKind::ArchiveEntry(entry)
            if entry.entry_index() == inner.index() && entry.path().value().is_some_and(|path| path == UNRELATED)
    )).ok_or("selected inner source")?;
    assert!(
        sources.iter_uses().any(|used| {
            used.source() == inner_source.id()
                && used.scope() == InputScope::ArchiveDirectoryMetadata
                && used.role() == SourceUseRole::Selector
                && matches!(used.location(), SourceLocation::Metadata { field } if field == "path")
        }),
        "the ineligible exact member uses its own original path as selector"
    );
    Ok(())
}
