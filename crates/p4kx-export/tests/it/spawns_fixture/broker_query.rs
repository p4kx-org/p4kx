// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::mission_broker_context::MissionBrokerContextSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordFailure, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries, ProvenanceMode,
};
use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, Filter, SourceLocation, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestValue,
};
use serde_json::Value;

use super::generator_query::{ENTITIES, LOCATIONS, NPCS, SHIPS};
use super::pool_query_checks::{check_game_text, registered_pool_rows};
use super::{
    REPUTATION_REWARD, array, broker, english, location, npc, reference, ships, structures,
    tag_records, template_parts, template_values,
};

type TestResult = Result<(), Box<dyn Error>>;

#[derive(Clone, Copy)]
enum Fault {
    None,
    Reward,
    Declaration,
}

#[test]
fn broker_context_pools_match_normal_catalog_first_winning_broker() -> TestResult {
    check(Fault::None)
}

#[test]
fn broker_context_pools_keep_unreadable_reward_and_exclude_failed_declaration() -> TestResult {
    check(Fault::Reward)?;
    check(Fault::Declaration)
}

fn check(fault: Fault) -> TestResult {
    let Fixture {
        _folder,
        archive,
        earlier_guid,
        later_guid,
    } = fixture(fault)?;
    let producer = producer(&archive, fault)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?
            .with_provenance_mode(ProvenanceMode::Full);
    let parsed = session.parsed(None)?;
    parsed.with_mission_broker_context(CatalogRecordSelection::All, None, |snapshot| {
        assert!(matches!(
            snapshot.availability,
            DatasetAvailability::Available
        ));
        assert_eq!(
            snapshot.owners.len(),
            2,
            "both physical owners remain visible"
        );
        compare_pools(&snapshot, &producer)?;
        match fault {
            Fault::None => check_accepted(&snapshot, &producer, earlier_guid),
            Fault::Reward => check_unreadable_reward(&snapshot, &producer, earlier_guid),
            Fault::Declaration => {
                check_failed_declaration(&snapshot, &producer, earlier_guid, later_guid)
            }
        }
    })??;
    if matches!(fault, Fault::None) {
        check_later_owner(&archive, &parsed, later_guid)?;
    }
    if matches!(fault, Fault::Declaration) {
        parsed.with_mission_broker_context(
            CatalogRecordSelection::RecordIndex(earlier_guid),
            None,
            |snapshot| check_rejected_selection(&snapshot),
        )?;
    }
    Ok(())
}

fn check_later_owner(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    later_guid: u32,
) -> TestResult {
    let filter = Filter::new("later_guid");
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.filter = Some(&filter);
    let built = build_catalog(archive, options.with_full_provenance())?;
    let producer: Value = serde_json::from_slice(built.json())?;
    parsed.with_mission_broker_context(
        CatalogRecordSelection::RecordIndex(later_guid),
        None,
        |snapshot| {
            assert_eq!(snapshot.rows.len(), 1, "scoped physical Broker");
            assert_eq!(snapshot.owners.len(), 1, "no unselected original Broker");
            compare_pools(&snapshot, &producer)?;
            check_name(&snapshot, &producer, later_guid)
        },
    )?
}

struct Fixture {
    _folder: tempfile::TempDir,
    archive: Archive,
    earlier_guid: u32,
    later_guid: u32,
}

fn fixture(fault: Fault) -> Result<Fixture, Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let tags = tag_records();
    let later_guid = u32::try_from(tags.len())?;
    let earlier_guid = later_guid + 1;
    let data = tags
        .into_iter()
        .chain([
            entry(2, "later_guid", Fault::None),
            entry(1, "earlier_guid", fault),
            template_parts::owner(),
        ])
        .fold(
            structures::structures(DataForgeBuilder::new().version(8)),
            DataForgeBuilder::record,
        );
    let data = match fault {
        Fault::Reward => data.record(broker::reward_record().with_instance_index(u16::MAX)),
        _ => data,
    };
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", data.build()).encrypted())
        .entry(english())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok(Fixture {
        _folder: folder,
        archive,
        earlier_guid,
        later_guid,
    })
}

fn check_accepted(
    snapshot: &MissionBrokerContextSnapshot<'_>,
    producer: &Value,
    winner: u32,
) -> TestResult {
    assert_eq!(snapshot.rows.len(), 2);
    assert_eq!(
        snapshot.contextual_failures,
        [] as [CatalogRecordFailure<'_>; 0]
    );
    assert_eq!(
        snapshot.reward_failures,
        [] as [CatalogRecordFailure<'_>; 0]
    );
    check_name(snapshot, producer, winner)
}

fn check_unreadable_reward(
    snapshot: &MissionBrokerContextSnapshot<'_>,
    producer: &Value,
    winner: u32,
) -> TestResult {
    assert_eq!(
        snapshot.rows.len(),
        2,
        "a recoverable Reward keeps both contexts"
    );
    assert_eq!(
        snapshot.contextual_failures,
        [] as [CatalogRecordFailure<'_>; 0]
    );
    assert_eq!(snapshot.reward_failures.len(), 1);
    let row = snapshot
        .rows
        .iter()
        .find(|row| row.broker().physical().record_index() == winner)
        .ok_or("Broker with the unreadable reward dependency")?;
    let [result] = row.reputation_results() else {
        return Err("expected one original reward result".into());
    };
    let [reward] = result.amounts() else {
        return Err("expected one original reward amount".into());
    };
    assert_eq!(
        reward.amount().ok_or("retained reward amount")?.state(),
        SourceValueState::Unreadable
    );
    check_name(snapshot, producer, winner)
}

fn check_failed_declaration(
    snapshot: &MissionBrokerContextSnapshot<'_>,
    producer: &Value,
    rejected: u32,
    winner: u32,
) -> TestResult {
    assert_eq!(
        snapshot.rows.len(),
        1,
        "a required declaration prevents its context"
    );
    let [failure] = snapshot.contextual_failures.as_slice() else {
        return Err("expected one required Broker declaration failure".into());
    };
    assert_eq!(failure.record().index(), rejected);
    assert_eq!(
        snapshot.reward_failures,
        [] as [CatalogRecordFailure<'_>; 0]
    );
    check_name(snapshot, producer, winner)
}

fn check_rejected_selection(snapshot: &MissionBrokerContextSnapshot<'_>) {
    assert!(matches!(
        snapshot.availability,
        DatasetAvailability::Available
    ));
    assert_eq!(
        snapshot.owners.len(),
        1,
        "the original rejected Broker remains visible"
    );
    assert_eq!(snapshot.contextual_failures.len(), 1);
    assert!(snapshot.rows.is_empty());
    assert!(snapshot.ship_spawns.is_empty());
    assert!(snapshot.npc_spawns.is_empty());
    assert!(snapshot.entity_spawns.is_empty());
    assert!(snapshot.location_queries.is_empty());
}

fn producer(archive: &Archive, fault: Fault) -> Result<Value, Box<dyn Error>> {
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    let normal = build_catalog(archive, options.with_full_provenance());
    let built = if matches!(fault, Fault::None) {
        normal.map_err(|error| error.to_string())?
    } else {
        assert!(
            matches!(normal, Err(CatalogError::IncompleteExport(_))),
            "an original required input prevents Normal publication"
        );
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        build_catalog(archive, options.with_full_provenance())?
    };
    Ok(serde_json::from_slice(built.json())?)
}

fn compare_pools(snapshot: &MissionBrokerContextSnapshot<'_>, producer: &Value) -> TestResult {
    let contexts = producer
        .pointer("/configurations/missionBrokerEntries/rows")
        .and_then(Value::as_array)
        .ok_or("producer Broker configurations")?;
    assert_eq!(
        snapshot.rows.len(),
        contexts.len(),
        "same accepted Broker owners"
    );
    for (path, kind, bytes) in [
        (SHIPS, "ships", serde_json::to_vec(&snapshot.ship_spawns)?),
        (NPCS, "npcs", serde_json::to_vec(&snapshot.npc_spawns)?),
        (
            ENTITIES,
            "entities",
            serde_json::to_vec(&snapshot.entity_spawns)?,
        ),
        (
            LOCATIONS,
            "location",
            serde_json::to_vec(&snapshot.location_queries)?,
        ),
    ] {
        let rows: Vec<Value> = serde_json::from_slice(&bytes)?;
        let expected = registered_pool_rows(producer, contexts, path, kind)?;
        assert_eq!(rows.len(), 1, "only one admitted content row at {path}");
        assert_eq!(
            rows.iter().collect::<Vec<_>>(),
            expected,
            "producer rows at {path}"
        );
    }
    Ok(())
}

fn check_name(
    snapshot: &MissionBrokerContextSnapshot<'_>,
    producer: &Value,
    winner: u32,
) -> TestResult {
    let name = snapshot.npc_spawns[0].groups[0].options[0]
        .embedded_name
        .as_ref()
        .ok_or("retained NPC name")?;
    let reference = name
        .given_name
        .value()
        .ok_or("protected NPC name reference")?;
    let row = snapshot
        .game_texts
        .iter()
        .find(|row| row.id().value() == Some(reference))
        .ok_or("NPC name in the same result")?;
    check_game_text(producer, row)?;
    let origin = name.given_name.origin();
    assert_eq!(origin.contains_encrypted_source(), Some(true));
    let mut own_name_uses = 0;
    for reference in origin.source_uses_refs() {
        let used = snapshot
            .source_inventory
            .iter_uses()
            .find(|used| used.id() == reference)
            .ok_or("contributing name evidence in this result")?;
        if let SourceLocation::Subresource {
            namespace, indices, ..
        } = used.location()
            && namespace == "dataforgeValue"
        {
            assert_eq!(
                indices.first(),
                Some(&u64::from(winner)),
                "the emitted name uses the first accepted canonical Broker"
            );
            own_name_uses += 1;
        }
    }
    assert!(
        own_name_uses > 0,
        "the name has original Broker field evidence"
    );
    Ok(())
}

fn entry(guid: u128, name: &str, fault: Fault) -> TestRecord {
    let rejected = matches!(fault, Fault::Declaration);
    let group = if rejected {
        "rejected group"
    } else {
        "shared group"
    };
    let limit = if rejected { 7 } else { 2 };
    let mut properties = vec![
        ships("Ship", "", Some(limit)),
        npc::npcs("Npc", "", Some((group, npc::NpcKind::Full))),
        npc::entities("Entity", "", Some(group)),
        location("Location", "", Some((1, limit))),
    ];
    if rejected {
        properties.push(template_values::mystery_location("Unsupported", true));
    }
    let mut record = broker::entry(
        guid,
        name,
        &format!("libs/foundry/records/missionbroker/{name}.xml"),
    )
    .value("properties", array(properties));
    if matches!(fault, Fault::Reward) {
        let amount = TestInstance::new("ReputationAmountEntry")
            .value("reward", reference(REPUTATION_REWARD));
        let result = TestInstance::new("ReputationResultSet")
            .value("reputationAmounts", array(vec![TestValue::Struct(amount)]));
        record = record.value(
            "missionResultReputationRewards",
            array(vec![TestValue::Struct(result)]),
        );
    }
    record
}
