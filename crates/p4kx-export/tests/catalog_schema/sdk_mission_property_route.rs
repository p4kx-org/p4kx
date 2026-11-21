// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::mission_properties::{
    DeclarationBinding, MissionPropertyConfiguration, MissionPropertyOwnerFamily,
    MissionPropertySelection, MissionPropertySnapshot, PropertySelection, SourcePayload,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, Language, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, InputScope, PhysicalSourceKind};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::Value;

use super::{Route, TestResult};

pub(super) const DATASET: &str = "/configurations/missionProperties";
const BROKER: MissionPropertyOwnerFamily = MissionPropertyOwnerFamily::MissionBrokerEntries;
const TEMPLATE: MissionPropertyOwnerFamily = MissionPropertyOwnerFamily::ContractTemplates;
const BROKER_TYPE: &str = "MissionBrokerEntry";
const TEMPLATE_TYPE: &str = "ContractTemplate";
const PROPERTY: &str = "MissionProperty";
const OBJECTIVE: &str = "MissionObjective";

pub(super) fn read() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("mission_properties.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"FixtureLocale=Unrelated locale\n",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog_from(&archive, &entry, &options)?.json())?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    let mut rows = Vec::new();
    for family in [TEMPLATE, BROKER] {
        let selection = MissionPropertySelection::new(family, CatalogRecordSelection::All);
        rows.extend(parsed.with_mission_properties(selection, None, |snapshot| {
            check_family(&catalog, family, snapshot)
        })??);
    }
    let exact = MissionPropertySelection::new(TEMPLATE, CatalogRecordSelection::RecordIndex(1));
    parsed
        .with_mission_properties(exact, None, |snapshot| check_exact_owner(&snapshot, &rows))??;
    let wrong = MissionPropertySelection::new(TEMPLATE, CatalogRecordSelection::RecordIndex(0));
    parsed.with_mission_properties(wrong, None, |snapshot| check_wrong_owner(&snapshot))?;
    assert_eq!(
        serde_json::to_value(&rows)?,
        *catalog
            .pointer("/configurations/missionProperties/rows")
            .ok_or("canonical property table")?,
        "one route across both families preserves canonical order, rows and per-value provenance",
    );
    Route::queried_rows(
        DATASET,
        &rows,
        "typed query, source arena and encrypted synthetic producer parity available; installed producer parity open",
    )
}

fn check_family(
    catalog: &Value,
    family: MissionPropertyOwnerFamily,
    snapshot: MissionPropertySnapshot<'_>,
) -> TestResult<Vec<MissionPropertyConfiguration>> {
    let definitions = match family {
        MissionPropertyOwnerFamily::ContractTemplates => "/definitions/contractTemplates/rows",
        MissionPropertyOwnerFamily::MissionBrokerEntries => {
            "/definitions/missionBrokerEntries/rows"
        }
        _ => return Err("unexpected owner family".into()),
    };
    let owners = catalog
        .pointer(definitions)
        .and_then(Value::as_array)
        .ok_or("canonical owners")?;
    assert_eq!(
        snapshot.sources.len(),
        owners.len(),
        "every admitted owner has one arena"
    );
    for source in &snapshot.sources {
        let index = u64::from(source.owner.physical().record_index());
        let original = owners
            .iter()
            .find(|owner| {
                owner.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(index)
            })
            .ok_or("canonical definition for the original physical owner")?;
        assert_eq!(
            serde_json::to_value(&source.arena)?,
            *original.get("source").ok_or("the original arena")?,
            "the exact owner Graph keeps canonical indexes, original shapes and origins",
        );
        check_property_links(&snapshot, index)?;
    }
    assert!(
        snapshot.coverage.unreadable_headers.is_empty()
            && snapshot.coverage.unreadable_bodies.is_empty()
            && snapshot.coverage.incomplete_fields.is_empty(),
        "all original owners and retained fields are readable",
    );
    assert!(
        snapshot
            .source_inventory
            .iter_sources()
            .all(|source| match source.kind() {
                PhysicalSourceKind::ArchiveEntry(entry) => entry
                    .path()
                    .value()
                    .is_none_or(|path| path.as_str() != Language::English.global_ini_path()),
                PhysicalSourceKind::ArchiveEntryProof(entry) => entry.entry_index().get() == 0,
                _ => true,
            }),
        "the original owner closure reads no locale payloads",
    );
    Ok(snapshot.rows)
}

fn check_property_links(snapshot: &MissionPropertySnapshot<'_>, index: u64) -> TestResult<()> {
    let source = snapshot
        .sources
        .iter()
        .find(|source| u64::from(source.owner.physical().record_index()) == index)
        .ok_or("the requested source arena")?;
    let mut found = 0;
    for row in snapshot
        .rows
        .iter()
        .filter(|row| u64::from(row.owner().physical().record_index()) == index)
    {
        let DeclarationBinding::Present { field } = row.declaration() else {
            return Err("the fixture has an original property declaration".into());
        };
        let field = source
            .arena
            .field(*field)
            .ok_or("typed declaration resolves in the arena")?;
        assert!(
            matches!(
                field.name().value().copied(),
                Some("contractProperties" | "properties")
            ),
            "the binding keeps the original property field",
        );
        for property in row.properties() {
            let value = source
                .arena
                .value(*property)
                .ok_or("typed original property node")?;
            let cell = value.original().value();
            assert!(
                matches!(cell.value(), Some(SourcePayload::Struct { .. })),
                "the property link resolves to the exact original structure",
            );
            let origin = cell.origin();
            assert_eq!(
                origin.contains_encrypted_source(),
                Some(true),
                "encrypted original property"
            );
            assert!(
                origin.source_set_complete(),
                "complete original source receipt"
            );
            assert!(
                snapshot.source_inventory.iter_uses().any(|usage| {
                    usage.scope() == InputScope::Payload
                        && origin.source_uses_refs().contains(usage.id())
                }),
                "each original property source resolves inside the selective inventory",
            );
            found += usize::from(matches!(row.selection(), PropertySelection::Selected));
        }
    }
    assert_eq!(found, 1, "the owner has one selected root property");
    Ok(())
}

fn check_exact_owner(
    snapshot: &MissionPropertySnapshot<'_>,
    all: &[MissionPropertyConfiguration],
) -> TestResult<()> {
    let [source] = snapshot.sources.as_slice() else {
        return Err("the exact Record selection must return only its physical arena".into());
    };
    assert_eq!(
        source.owner.physical().record_index(),
        1,
        "physical Template owner"
    );
    let expected: Vec<_> = all
        .iter()
        .filter(|row| row.owner().physical().record_index() == 1)
        .collect();
    assert_eq!(
        serde_json::to_value(&snapshot.rows)?,
        serde_json::to_value(expected)?,
        "the exact owner keeps its rows and source coordinates",
    );
    Ok(())
}

fn check_wrong_owner(snapshot: &MissionPropertySnapshot<'_>) {
    assert!(
        snapshot.rows.is_empty(),
        "the other Record family is excluded"
    );
    assert!(
        snapshot.sources.is_empty(),
        "no arena is read for an excluded Record"
    );

    assert_eq!(
        snapshot.coverage.unreadable_bodies.as_slice(),
        &[] as &[u32],
        "the other family is excluded before body reads",
    );
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(TEMPLATE_TYPE)
                .field("notForRelease", TestType::Bool)
                .array("contractProperties", pointer(PROPERTY))
                .array("objectiveTokens", pointer(OBJECTIVE)),
        )
        .structure(
            TestStruct::new(BROKER_TYPE)
                .field("debugName", TestType::String)
                .array("properties", pointer(PROPERTY)),
        )
        .structure(
            TestStruct::new(PROPERTY)
                .field("missionVariableName", TestType::String)
                .field("extendedTextToken", TestType::String),
        )
        .structure(
            TestStruct::new(OBJECTIVE)
                .field("id", TestType::Guid)
                .array("properties", pointer(PROPERTY)),
        )
        .record(
            TestRecord::new(
                3,
                BROKER_TYPE,
                "MissionBrokerEntry.Original",
                "libs/foundry/records/missionbroker/original.xml",
            )
            .value("debugName", TestValue::text("Broker"))
            .value("properties", properties("BrokerValue")),
        )
        .record(template(2, "first"))
        .record(template(2, "second"))
        .build()
}

fn template(id: u128, name: &str) -> TestRecord {
    let objective =
        TestInstance::new(OBJECTIVE).value("properties", properties("OutsideSelection"));
    TestRecord::new(
        id,
        TEMPLATE_TYPE,
        format!("ContractTemplate.{name}"),
        format!("libs/foundry/records/contracts/contracttemplates/{name}.xml"),
    )
    .value("notForRelease", TestValue::Bool(false))
    .value("contractProperties", properties(name))
    .value(
        "objectiveTokens",
        TestValue::Array(vec![TestValue::Struct(objective)]),
    )
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn properties(name: &str) -> TestValue {
    let property = TestInstance::new(PROPERTY)
        .value("missionVariableName", TestValue::text(name))
        .value("extendedTextToken", TestValue::text(""));
    TestValue::Array(vec![TestValue::Struct(property)])
}
