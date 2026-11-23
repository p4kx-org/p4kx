// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, ContractGeneratorDefinition,
    ContractSnapshot, SourcePayload, SourceValueRef, with_contracts_from,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, InputScope, SourceLocation};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestValue,
};
use serde::Serialize;
use serde_json::Value;

use super::{
    CONTRACT, GENERATOR, array, location, npc, overrides, property, ships, structures, tag_records,
    text,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(crate) const SHIPS: &str = "/contractShipSpawns";
pub(crate) const ENTITIES: &str = "/contractEntitySpawns";
pub(crate) const LOCATIONS: &str = "/contractLocationQueries";
pub(crate) const NPCS: &str = "/contractNpcSpawns";

const NPC_WEIGHT: f32 = 0.625;

fn npc_with_weight() -> TestValue {
    let option = TestInstance::new("SpawnDescription_NPCOption")
        .value("weight", TestValue::Single(NPC_WEIGHT));
    let group = TestInstance::new("SpawnDescription_NPC_Group")
        .value("Name", text("Fixture Guard Group"))
        .value("options", array(vec![TestValue::Struct(option)]));
    property(
        "Guard",
        "",
        TestInstance::new("MissionPropertyValue_NPCSpawnDescriptions")
            .value("spawnDescriptions", array(vec![TestValue::Struct(group)])),
    )
}

fn generator() -> TestRecord {
    let properties = vec![
        ships("WaveA", "", Some(3)),
        ships("WaveB", "", Some(1)),
        ships("WaveAlias", "", Some(3)),
        npc::entities("FirstCrate", "", Some("First")),
        npc::entities("SecondCrate", "", Some("Second")),
        location("SinglePlace", "", None),
        location("SeveralPlaces", "", Some((1, 3))),
        npc_with_weight(),
    ];
    let contract = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(CONTRACT))
        .value("debugName", text("SpawnScope"))
        .value("paramOverrides", overrides(properties));
    let handler = TestInstance::new("ContractGeneratorHandler_Career")
        .value("debugName", text("SpawnHandler"))
        .value("contracts", array(vec![TestValue::Struct(contract)]));
    TestRecord::new(
        GENERATOR,
        "ContractGenerator",
        "ContractGenerator.SpawnScope",
        "libs/foundry/records/contracts/contractgenerator/spawn_scope.xml",
    )
    .value("generators", array(vec![TestValue::Struct(handler)]))
}

pub(crate) fn with_catalog_parity<T>(
    inspect: impl FnOnce(&Value, ContractSnapshot<'_>) -> TestResult<T>,
) -> TestResult<T> {
    let records = tag_records();
    let record_index = u32::try_from(records.len())?;
    let forge = records
        .into_iter()
        .chain([generator()])
        .fold(
            structures::structures(DataForgeBuilder::new().version(8)),
            DataForgeBuilder::record,
        )
        .build();
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::zstd("Data/Game2.dcb", forge).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let input = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &input, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    with_contracts_from(
        &archive,
        &input,
        CatalogRecordSelection::RecordIndex(record_index),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            owner_evidence(&catalog, &snapshot, record_index)?;
            check_expected_rows(&catalog, &snapshot)?;
            inspect(&catalog, snapshot)
        },
    )?
}

fn owner_evidence(
    catalog: &Value,
    snapshot: &ContractSnapshot<'_>,
    record_index: u32,
) -> TestResult {
    let [owner] = snapshot.generators.as_slice() else {
        return Err("one physical Generator selected by the SDK".into());
    };
    assert_eq!(
        owner.header().record_index().value().copied(),
        Some(record_index),
        "the typed query selected the fixture's actual physical Generator"
    );
    assert!(
        snapshot.dependency_generators.is_empty()
            && snapshot.coverage.dependencies.forward_records.is_empty()
            && snapshot.coverage.projection_failures.is_empty(),
        "only the intended physical owner supplies spawn projections"
    );
    assert_eq!(
        Some(&wire_value(owner.source())?),
        catalog.pointer("/definitions/contractGenerators/rows/0/source"),
        "the raw Generator arena, authored order and per-field origins match the producer"
    );
    for (kind, field) in [
        ("SpawnDescription_Ship", "weight"),
        ("SpawnDescription_Entity", "amount"),
        ("MissionPropertyValue_Locations", "maxLocationsToFind"),
        ("SpawnDescription_NPCOption", "weight"),
    ] {
        verify_operand(catalog, snapshot, owner, record_index, kind, field)?;
    }
    Ok(())
}

fn verify_operand(
    catalog: &Value,
    snapshot: &ContractSnapshot<'_>,
    owner: &ContractGeneratorDefinition<'_>,
    record_index: u32,
    kind: &str,
    field: &str,
) -> TestResult {
    let reference = original_operand(owner, kind, field).ok_or("actual numeric spawn operand")?;
    let operand = owner
        .source()
        .value(reference)
        .ok_or("linked original spawn operand")?;
    assert!(
        matches!(operand.value().value(), Some(SourcePayload::Number { .. })),
        "the original {kind}.{field} retains a typed number"
    );
    let origin = operand.value().origin();
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the exact original {kind}.{field} came from the encrypted Entry"
    );
    assert!(
        origin.source_set_complete(),
        "the exact original {kind}.{field} has complete field evidence"
    );
    let used = snapshot.source_inventory.iter_uses()
        .find(|used| {
            origin.source_uses_refs().contains(used.id())
                && used.scope() == InputScope::Payload
                && matches!(used.location(), SourceLocation::Subresource { namespace, indices, field }
                    if namespace == "dataforgeValue"
                        && indices.as_slice() == [u64::from(record_index), reference.value_index()]
                        && field == "$value")
        })
        .ok_or("original DataForge operand use")?;
    let expected = serde_json::to_value(used)?;
    let catalog_uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("normal producer retained source uses")?;
    assert!(
        catalog_uses.contains(&expected),
        "the producer retains the same physical operand use"
    );
    Ok(())
}

fn original_operand(
    owner: &ContractGeneratorDefinition<'_>,
    kind: &str,
    field: &str,
) -> Option<SourceValueRef> {
    owner.source().values().iter().find_map(|node| {
        let SourcePayload::Struct {
            source_type,
            fields,
            ..
        } = node.value().value()?
        else {
            return None;
        };
        if source_type.value().copied() != Some(kind) {
            return None;
        }
        let field = fields
            .iter()
            .find(|candidate| candidate.name().value().copied() == Some(field))?;
        Some(field.value())
    })
}

fn check_expected_rows(catalog: &Value, snapshot: &ContractSnapshot<'_>) -> TestResult {
    assert_eq!(
        snapshot
            .npc_spawns
            .first()
            .and_then(|pool| pool.groups.first())
            .and_then(|group| group.options.first())
            .and_then(|option| option.weight),
        Some(NPC_WEIGHT),
        "the selected NPC spawn preserves the original numeric option weight"
    );
    for (dataset, expected_rows, actual) in [
        (SHIPS, 2, wire_value(&snapshot.ship_spawns)?),
        (ENTITIES, 2, wire_value(&snapshot.entity_spawns)?),
        (LOCATIONS, 2, wire_value(&snapshot.location_queries)?),
        (NPCS, 1, wire_value(&snapshot.npc_spawns)?),
    ] {
        assert_eq!(
            actual.as_array().map(Vec::len),
            Some(expected_rows),
            "the selected owner contributes the expected number of distinct {dataset} rows"
        );
        assert_eq!(
            Some(&actual),
            catalog.pointer(dataset),
            "{dataset} retains canonical content, deduplication and GUID order"
        );
    }
    Ok(())
}

fn wire_value(value: &impl Serialize) -> serde_json::Result<Value> {
    serde_json::from_slice(&serde_json::to_vec(value)?)
}

#[test]
fn physical_generator_reaches_the_normal_spawn_projections() -> TestResult {
    with_catalog_parity(|_, _| Ok(()))
}
