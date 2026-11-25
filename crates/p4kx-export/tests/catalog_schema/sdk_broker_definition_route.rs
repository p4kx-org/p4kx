// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::mission_definitions::{
    BrokerDefinition, MissionDefinitionFamily, MissionDefinitionRows, MissionDefinitionSelection,
};
use p4kx_export::catalog_queries::mission_properties::{SourcePayload, TemplateSourceRoot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use super::{Route, TestResult};

pub(in super::super) const DATASET: &str = "/definitions/missionBrokerEntries";

const BROKER_RECORD: u32 = 0;

pub(super) fn read(parsed: &ParsedCatalogQueries<'_>, catalog: &Value) -> TestResult<Route> {
    let selection = MissionDefinitionSelection::new(
        MissionDefinitionFamily::MissionBrokerEntries,
        CatalogRecordSelection::RecordIndex(BROKER_RECORD),
    );
    parsed.with_mission_definitions(selection, None, |snapshot| {
        let MissionDefinitionRows::MissionBrokerEntries(rows) = &snapshot.rows else {
            return Err("expected canonical Broker definitions".into());
        };
        let [broker] = rows.as_slice() else {
            return Err("expected one physical Broker definition".into());
        };
        let table = json!({"availability": snapshot.availability, "rows": rows});
        assert_eq!(
            Some(&table),
            catalog.pointer(DATASET),
            "the exact typed owner preserves the normal producer row, original arena and origins",
        );
        assert!(
            snapshot.coverage.unreadable_headers.is_empty()
                && snapshot.coverage.unreadable_bodies.is_empty()
                && snapshot.coverage.incomplete_fields.is_empty(),
            "the selected original Broker projects completely",
        );
        assert!(
            snapshot.game_texts.is_empty() && snapshot.available_languages.is_empty(),
            "the authored Locale is available as a source cell and needs no translation payload",
        );
        check_links(broker)?;
        check_title(broker)?;
        Route::queried_rows(
            DATASET,
            rows,
            "normal synthetic producer identity, arena, link, and origin parity; installed parity open",
        )
    })?
}

fn check_links(broker: &BrokerDefinition<'_>) -> TestResult {
    let header = broker.header();
    let guid = header.guid();
    assert_eq!(
        header.record_index().value(),
        Some(&BROKER_RECORD),
        "the physical Broker position"
    );
    let [flow] = broker.mission_flows() else {
        return Err("expected the original Flow declaration link".into());
    };
    let [properties] = broker.mission_properties() else {
        return Err("expected the original property declaration survey link".into());
    };
    for owner in [flow.owner(), properties.owner()] {
        let owner = owner.physical();
        assert_eq!(
            owner.record_index(),
            BROKER_RECORD,
            "links use the owning physical Record"
        );
        assert_eq!(
            owner.input_ref(),
            broker.input_ref(),
            "links refer to the same original DataForge"
        );
        assert_eq!(
            Some(&owner.record_guid().get()),
            guid.value(),
            "links retain the enclosing Record GUID"
        );
    }
    Ok(())
}

fn check_title(broker: &BrokerDefinition<'_>) -> TestResult {
    let source = broker.source();
    let field = source
        .roots()
        .iter()
        .map(TemplateSourceRoot::field)
        .find(|field| field.name().value().copied() == Some("title"))
        .ok_or("the original title field")?;
    let value = source
        .value(field.value())
        .ok_or("the original title occurrence")?
        .original()
        .value();
    assert!(
        matches!(value.value(), Some(SourcePayload::Locale { .. })),
        "the authored title remains a typed original Locale",
    );
    assert_eq!(
        value.origin().contains_encrypted_source(),
        Some(true),
        "the title key retains its encrypted DataForge source",
    );
    Ok(())
}

#[test]
fn broker_definitions_apply_canonical_admission_to_all_and_guid() -> TestResult {
    use std::time::UNIX_EPOCH;

    use p4kx_export::CatalogOptions;

    let (_directory, archive) = broker_admission_fixture()?;
    let entry = DataForgeEntry::read(&archive)?;
    let guid = entry
        .open()?
        .record(1)
        .ok_or("explicit leftover Record")?
        .guid();
    let catalog = serde_json::from_slice::<Value>(
        p4kx_export::build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    let normal = catalog
        .pointer(DATASET)
        .ok_or("complete Normal Broker definition table")?;
    check_broker_selection(
        &parsed,
        CatalogRecordSelection::All,
        &[0, 2],
        1,
        Some(normal),
    )?;
    let first = normal
        .pointer("/rows/0")
        .ok_or("the admitted GUID's Normal row")?;
    let scoped = json!({"availability": normal.get("availability"), "rows": [first]});
    check_broker_selection(
        &parsed,
        CatalogRecordSelection::Guid(guid),
        &[0],
        1,
        Some(&scoped),
    )?;
    check_broker_selection(
        &parsed,
        CatalogRecordSelection::RecordIndex(1),
        &[1],
        0,
        None,
    )
}

fn check_broker_selection(
    parsed: &ParsedCatalogQueries<'_>,
    selection: CatalogRecordSelection,
    expected_indices: &[u32],
    expected_skipped: usize,
    normal: Option<&Value>,
) -> TestResult {
    let selection =
        MissionDefinitionSelection::new(MissionDefinitionFamily::MissionBrokerEntries, selection);
    parsed.with_mission_definitions(selection, None, |snapshot| {
        let MissionDefinitionRows::MissionBrokerEntries(rows) = &snapshot.rows else {
            return Err("expected canonical Broker definitions".into());
        };
        let indices: Vec<u32> = rows
            .iter()
            .map(|value| {
                value
                    .header()
                    .record_index()
                    .value()
                    .copied()
                    .ok_or("missing physical Record")
            })
            .collect::<Result<_, _>>()?;
        assert_eq!(
            indices, expected_indices,
            "canonical admission retains the matching physical Records in order"
        );
        assert_eq!(
            snapshot.coverage.skipped_leftovers, expected_skipped,
            "only canonical selections apply the leftover policy"
        );
        assert!(
            snapshot.available_languages.is_empty() && snapshot.game_texts.is_empty(),
            "the authored Broker title remains an untranslated arena value"
        );
        if let Some(expected) = normal {
            assert_eq!(
                &json!({"availability": snapshot.availability, "rows": rows}),
                expected,
                "the admitted SDK rows and their original source arenas equal Normal output"
            );
        }
        Ok(())
    })?
}

#[test]
fn broker_definitions_keep_guid_row_order_and_physical_fault_order() -> TestResult {
    let (_directory, archive) = broker_fixture([
        broker_record(6, "DamagedLateGuid").with_instance_index(u16::MAX),
        broker_record(3, "FirstAlias"),
        broker_record(1, "DamagedEarlyGuid").with_instance_index(u16::MAX),
        broker_record(2, "Second"),
        broker_record(3, "SecondAlias"),
        broker_record(6, "Dev_Damaged").with_instance_index(u16::MAX),
    ])?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let guid = "00000000-0000-0000-0000-000000000006".parse()?;
    for (records, expected_rows, expected_faults, skipped) in [
        (CatalogRecordSelection::All, &[3, 1, 4][..], &[0, 2][..], 1),
        (CatalogRecordSelection::Guid(guid), &[][..], &[0][..], 1),
        (CatalogRecordSelection::RecordIndex(5), &[][..], &[5][..], 0),
    ] {
        let selection =
            MissionDefinitionSelection::new(MissionDefinitionFamily::MissionBrokerEntries, records);
        parsed.with_mission_definitions(selection, None, |snapshot| -> TestResult {
            let MissionDefinitionRows::MissionBrokerEntries(rows) = &snapshot.rows else {
                return Err("expected canonical Broker definitions".into());
            };
            let positions: Vec<_> = rows
                .iter()
                .map(|row| row.header().record_index().value().copied())
                .collect::<Option<_>>()
                .ok_or("readable physical Record positions")?;
            assert_eq!(positions, expected_rows, "GUID order with physical ties");
            assert_eq!(
                snapshot.coverage.unreadable_bodies, expected_faults,
                "faults preserve original Record order",
            );
            assert_eq!(
                snapshot.coverage.skipped_leftovers, skipped,
                "canonical admission excludes the development owner before reading its body",
            );
            Ok(())
        })??;
    }
    Ok(())
}

fn broker_admission_fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    broker_fixture([
        broker_record(1, "First"),
        broker_record(1, "Dev_Excluded"),
        broker_record(2, "Second"),
    ])
}

const BROKER: &str = "MissionBrokerEntry";

fn broker_record(guid: u128, label: &str) -> TestRecord {
    TestRecord::new(
        guid,
        BROKER,
        format!("{BROKER}.{label}"),
        format!("libs/foundry/records/missionbroker/{label}.xml"),
    )
    .value("title", TestValue::text("@fixture_broker_title"))
}

fn broker_fixture(
    records: impl IntoIterator<Item = TestRecord>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let mut database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(BROKER).field("title", TestType::Locale));
    for record in records {
        database = database.record(record);
    }
    let database = database.build();
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((directory, archive))
}
