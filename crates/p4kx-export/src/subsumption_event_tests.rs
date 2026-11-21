// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, CancelToken, Guid, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntry,
};
use serde_json::Value;

use super::{
    SubsumptionEvent, SubsumptionEventControl, SubsumptionEventField, SubsumptionEventFormat,
    SubsumptionEventValue,
};
use crate::catalog_queries::subsumption_events::{
    SubsumptionEventLookup, SubsumptionEventSelection, read_subsumption_events,
};
use crate::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog};

#[path = "subsumption_event_witness.rs"]
mod witness;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const OLDER: &str = "Data/Libs/Subsumption/Events/fixture/older.xml";
const NEWER: &str = "Data/Libs/Subsumption/Events/fixture/newer.xml";
const UNKNOWN: &str = "Data/Libs/Subsumption/Events/fixture/unknown.xml";
const BROKEN: &str = "Data/Libs/Subsumption/Events/fixture/broken.xml";
const ID: &str = "00000000-0000-0000-0000-000000000001";
const OTHER: &str = "00000000-0000-0000-0000-000000000002";
const MISSING: &str = "00000000-0000-0000-0000-000000000003";

fn fixture(broken: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    let mut builder = ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            DataForgeBuilder::new().version(8).build(),
        ))
        .entry(TestEntry::stored(
            OLDER,
            format!(
                "<Subsumption><Event ID='{ID}' Name='Earlier' Range='01.5' \
                RequiresVisual='False'><Future Extra='original'/></Event></Subsumption>"
            ),
        ))
        .entry(TestEntry::stored(NEWER, binary(ID, "Later")).encrypted())
        .entry(
            TestEntry::stored(UNKNOWN, binary(OTHER, "UnknownOrigin"))
                .encrypted()
                .with_encryption_field(7),
        )
        .entry(
            TestEntry::stored(
                "Data/Libs/Subsumption/EventsExtra/ignored.xml",
                "outside the family",
            )
            .encrypted(),
        );
    if broken {
        builder = builder.entry(TestEntry::stored(BROKEN, "<Subsumption"));
    }
    builder.write_to(&path)?;
    Ok((
        folder,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn binary(id: &str, name: &str) -> Vec<u8> {
    TestElement::new("Subsumption")
        .child(
            TestElement::new("Event")
                .attribute("ID", format!("{{{id}}}"))
                .attribute("Name", name)
                .attribute("Range", "1.5")
                .attribute("RequiresVisual", "False")
                .child(TestElement::new("Future").attribute("Extra", "retained")),
        )
        .child(TestElement::new("Event").attribute("Range", "unknown"))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn control(
    event: &SubsumptionEvent,
    field: SubsumptionEventField,
) -> TestResult<&SubsumptionEventControl> {
    event
        .controls()
        .iter()
        .find(|control| control.field() == field)
        .ok_or_else(|| "typed control".into())
}

#[test]
fn authored_id_keeps_renames_and_all_physical_declarations() -> TestResult {
    let (_folder, archive) = fixture(false)?;
    let id: Guid = ID.parse()?;
    let found = read_subsumption_events(&archive, SubsumptionEventSelection::EventId(id), None)?;
    assert_eq!(
        found.lookup,
        Some(SubsumptionEventLookup::Ambiguous),
        "two stored declarations share the identifier"
    );
    let [old, new] = found.rows.as_slice() else {
        return Err("both physical event documents".into());
    };
    assert_ne!(
        old.identity().entry_index(),
        new.identity().entry_index(),
        "physical entries remain distinct"
    );
    let old_event = old.events().first().ok_or("first document's event")?;
    let new_event = new.events().first().ok_or("replacement document's event")?;
    assert_eq!(
        old_event.id().value(),
        new_event.id().value(),
        "renaming preserves the canonical authored ID"
    );
    assert_ne!(
        old_event.name().value(),
        new_event.name().value(),
        "both original names are retained"
    );
    for (event, encrypted) in [(old_event, false), (new_event, true)] {
        let range = control(event, SubsumptionEventField::Range)?.value();
        assert_eq!(
            range.value(),
            Some(&SubsumptionEventValue::Number(1.5)),
            "text and binary values agree"
        );
        assert_eq!(
            range.origin().contains_encrypted_source(),
            Some(encrypted),
            "every typed value carries its own source marker"
        );
    }
    let selected = read_subsumption_events(
        &archive,
        SubsumptionEventSelection::EntryIndex(new.identity().entry_index()),
        None,
    )?;
    assert_eq!(
        selected.lookup,
        Some(SubsumptionEventLookup::Unique),
        "a physical selection narrows aliases"
    );
    assert_eq!(
        selected.entries.selected, 1,
        "the physical query reads only the requested entry"
    );
    Ok(())
}

#[test]
fn path_query_keeps_unknown_origin_order_and_malformed_fields() -> TestResult {
    let (_folder, archive) = fixture(false)?;
    let snapshot = read_subsumption_events(
        &archive,
        SubsumptionEventSelection::ArchivePath(r"data\LIBS\Subsumption\Events\fixture\UNKNOWN.xml"),
        None,
    )?;
    let [row] = snapshot.rows.as_slice() else {
        return Err("one normalized path match".into());
    };
    assert_eq!(
        row.source_format().value(),
        Some(&SubsumptionEventFormat::CryXmlB),
        "the selected source retains its actual encoding"
    );
    let [known, malformed] = row.events() else {
        return Err("both direct event declarations".into());
    };
    let index = usize::try_from(
        *known
            .element_index()
            .value()
            .ok_or("authored event locator")?,
    )?;
    assert_eq!(
        row.elements()
            .get(index + 1)
            .and_then(|node| node.name().value())
            .map(String::as_str),
        Some("Future"),
        "unknown descendants retain their original position"
    );
    assert_eq!(
        known.id().origin().contains_encrypted_source(),
        None,
        "unknown source encryption remains unknown"
    );
    assert_eq!(
        malformed.id().state(),
        SourceValueState::Absent,
        "missing ID stays absent"
    );
    assert_eq!(
        control(malformed, SubsumptionEventField::Range)?
            .value()
            .state(),
        SourceValueState::Invalid,
        "an invalid number is retained as a typed state"
    );
    Ok(())
}

#[test]
fn absent_and_unavailable_id_lookups_remain_distinct() -> TestResult {
    for (broken, expected) in [
        (false, SubsumptionEventLookup::NotFound),
        (true, SubsumptionEventLookup::Unavailable),
    ] {
        let (_folder, archive) = fixture(broken)?;
        let missing = SubsumptionEventSelection::EventId(MISSING.parse()?);
        let selected = read_subsumption_events(&archive, missing, None)?;
        assert_eq!(
            selected.lookup,
            Some(expected),
            "an unreadable candidate precludes a proven absence"
        );
        assert_eq!(
            selected.entries.failed_projections,
            usize::from(broken),
            "the XML failure is visible"
        );
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(
            matches!(
                read_subsumption_events(&archive, missing, Some(&cancel)),
                Err(CatalogError::Cancelled)
            ),
            "cancelled reads return no completed result"
        );
    }
    Ok(())
}

#[test]
fn sdk_event_documents_match_synthetic_catalog_output() -> TestResult {
    let (_folder, archive) = fixture(false)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let selected = read_subsumption_events(&archive, SubsumptionEventSelection::All, None)?;
    let expected = catalog
        .pointer("/definitions/subsumptionEvents/rows")
        .ok_or("canonical event dataset")?;
    let rows: Value = serde_json::from_slice(&serde_json::to_vec(&selected.rows)?)?;
    assert_eq!(
        without_registry_ids(expected.clone()),
        without_registry_ids(rows),
        "SDK and Catalog publish the same values and origin states"
    );
    Ok(())
}

fn without_registry_ids(mut value: Value) -> Value {
    match &mut value {
        Value::Array(values) => {
            for row in values {
                *row = without_registry_ids(row.take());
            }
        }
        Value::Object(object) => {
            object.remove("sourceUsesRefs");
            for row in object.values_mut() {
                *row = without_registry_ids(row.take());
            }
        }
        _ => {}
    }
    value
}
