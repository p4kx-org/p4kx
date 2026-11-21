// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::equipment::EquipmentSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    OriginalValueErrorFacts,
};
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestStruct, TestType, TestValue, entity_schema,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const OWNER: u128 = 0xd653;
const SHIELD: &str = "SCItemShieldGeneratorParams";
const SHIELD_ROWS: &str = "/configurations/shieldGenerators/rows";
const QUANTUM: &str = "SCItemQuantumDriveParams";
const QUANTUM_ROWS: &str = "/configurations/quantumDrives/rows";

#[test]
fn ship_system_queries_keep_all_physical_records_with_the_same_guid() -> TestResult {
    let database = systems()
        .record(owner("Alpha", 8.0).record())
        .record(owner("Beta", 16.0).record());
    let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let guid = common::guid(OWNER).parse()?;
    for (selection, indices) in [
        (CatalogRecordSelection::All, &[0, 1][..]),
        (CatalogRecordSelection::Guid(guid), &[0, 1][..]),
        (CatalogRecordSelection::RecordIndex(0), &[0][..]),
        (CatalogRecordSelection::RecordIndex(1), &[1][..]),
    ] {
        parsed.with_shield_generators(selection, None, |snapshot| -> TestResult {
            verify_rows(&snapshot, &catalog, SHIELD_ROWS, indices)
        })??;
        parsed.with_quantum_drives(selection, None, |snapshot| -> TestResult {
            verify_rows(&snapshot, &catalog, QUANTUM_ROWS, indices)
        })??;
    }
    Ok(())
}

#[test]
fn ship_system_queries_distinguish_absent_families_from_unreadable_candidates() -> TestResult {
    let base = entity_schema(DataForgeBuilder::new().version(8)).record(item("NoSystems").record());
    let (_folder, archive) = common::open(&base, DcbOrigin::Plain)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let selection = CatalogRecordSelection::All;
    parsed.with_shield_generators(selection, None, |snapshot| verify_absence(&snapshot))?;
    parsed.with_quantum_drives(selection, None, |snapshot| verify_absence(&snapshot))?;

    let mut pointer = (u32::MAX - 1).to_le_bytes().to_vec();
    pointer.extend_from_slice(&0_u32.to_le_bytes());
    let database = systems().record(
        item("UnclassifiableSystem")
            .component(TestValue::Raw(pointer))
            .record(),
    );
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    parsed.with_shield_generators(selection, None, |snapshot| verify_incomplete(&snapshot))?;
    parsed.with_quantum_drives(selection, None, |snapshot| verify_incomplete(&snapshot))?;
    Ok(())
}

#[test]
fn shield_query_retains_an_outside_owner_weak_pointer_with_complete_coverage() -> TestResult {
    let (_folder, archive) = common::open(&outside_owner_shield(), DcbOrigin::Encrypted)?;
    let produced = build_catalog(&archive, &common::options())?;
    assert!(
        produced.summary().export_health().is_complete(),
        "the outside-owner target is an informational original observation",
    );
    let catalog: Value = serde_json::from_slice(produced.json())?;
    let [expected] = proof::rows(&catalog, SHIELD_ROWS)? else {
        return Err("one canonical shield row with the original pointer".into());
    };
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    parsed.with_shield_generators(
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one SDK shield row with the original pointer".into());
            };
            assert!(
                snapshot.coverage().incomplete_fields.is_empty(),
                "the raw pointer target had a readable physical identity",
            );
            let Some((struct_index, instance_index)) =
                row.additional_fields().values().iter().find_map(|node| {
                    match node.original_error()? {
                        OriginalValueErrorFacts::OutsideRecord {
                            struct_index,
                            instance_index,
                        } => Some((struct_index, instance_index)),
                        _ => None,
                    }
                })
            else {
                return Err("the original outside-owner pointer coordinates".into());
            };
            assert!(struct_index.value().is_some() && struct_index.origin().source_set_complete());
            assert!(
                instance_index.value().is_some() && instance_index.origin().source_set_complete()
            );
            let sdk = json!({"provenance": snapshot.source_inventory()});
            let row = serde_json::from_slice(&serde_json::to_vec(row)?)?;
            assert_eq!(
                proof::with_origins(row, &sdk)?,
                proof::with_origins(expected.clone(), &catalog)?,
                "the SDK and producer retain the same pointer and its per-field sources",
            );
            Ok(())
        },
    )??;
    Ok(())
}

fn outside_owner_shield() -> DataForgeBuilder {
    const TARGET: &str = "ShieldExternal";
    const LINK: &str = "opaqueLink";
    const INSTANCE: &str = "outside-shield-owner";
    entity_schema(DataForgeBuilder::new().version(8))
        .structure(shield_type().field(LINK, TestType::WeakPointer(TARGET.to_owned())))
        .structure(TestStruct::new(TARGET).field("marker", TestType::Bool))
        .instance(
            INSTANCE,
            TestInstance::new(TARGET).value("marker", TestValue::Bool(true)),
        )
        .record(
            item("OutsideOwner")
                .component(TestValue::Struct(
                    TestInstance::new(SHIELD)
                        .value("MaxShieldHealth", TestValue::Single(8.0))
                        .value(LINK, TestValue::pointer(INSTANCE)),
                ))
                .record(),
        )
}

fn shield_type() -> TestStruct {
    TestStruct::new(SHIELD)
        .parent("DataForgeComponentParams")
        .field("MaxShieldHealth", TestType::Single)
}

fn systems() -> DataForgeBuilder {
    entity_schema(DataForgeBuilder::new().version(8))
        .structure(shield_type())
        .structure(
            TestStruct::new(QUANTUM)
                .parent("DataForgeComponentParams")
                .field("jumpRange", TestType::Single),
        )
}

fn item(name: &str) -> TestEntity {
    TestEntity::new(
        OWNER,
        format!("FixtureShipSystem{name}"),
        format!("libs/foundry/records/entities/scitem/ships/fixture_system_{name}.xml"),
    )
    .attach_type("Misc")
}

fn owner(name: &str, value: f32) -> TestEntity {
    item(name)
        .component(TestValue::Struct(
            TestInstance::new(SHIELD).value("MaxShieldHealth", TestValue::Single(value)),
        ))
        .component(TestValue::Struct(
            TestInstance::new(QUANTUM).value("jumpRange", TestValue::Single(value)),
        ))
}

fn verify_rows<Row: Serialize>(
    snapshot: &EquipmentSnapshot<'_, Row>,
    catalog: &Value,
    dataset: &str,
    owners: &[usize],
) -> TestResult {
    let canonical = proof::rows(catalog, dataset)?;
    assert_eq!(canonical.len(), 2, "{dataset} has two physical owners");
    assert_eq!(
        snapshot.rows().len(),
        owners.len(),
        "{dataset} has every selected physical owner",
    );
    assert!(
        snapshot.coverage().incomplete_fields.is_empty(),
        "{dataset} has readable original fields",
    );
    let sdk = json!({"provenance": snapshot.source_inventory()});
    for (row, &owner) in snapshot.rows().iter().zip(owners) {
        let expected = canonical.get(owner).ok_or("canonical physical owner")?;
        assert_eq!(
            proof::at(expected, "/ownerRecordIndex/value")?,
            &json!(owner),
            "canonical {dataset} row identifies its physical Record",
        );
        let row = serde_json::from_slice(&serde_json::to_vec(row)?)?;
        assert_eq!(
            proof::with_origins(row, &sdk)?,
            proof::with_origins(expected.clone(), catalog)?,
            "canonical {dataset} row and origins match selected physical owner {owner}",
        );
    }
    Ok(())
}

fn verify_absence<Row>(snapshot: &EquipmentSnapshot<'_, Row>) {
    assert!(
        matches!(snapshot.availability(), DatasetAvailability::Absent),
        "the source has no declaration for this component family",
    );
    assert!(snapshot.rows().is_empty(), "the family has no rows");
    assert!(
        snapshot.coverage().incomplete_fields.is_empty(),
        "the absence is established without unreadable source fields",
    );
}

fn verify_incomplete<Row>(snapshot: &EquipmentSnapshot<'_, Row>) {
    assert!(
        matches!(snapshot.availability(), DatasetAvailability::Available),
        "the source declares this component family",
    );
    assert!(
        snapshot.rows().is_empty(),
        "no readable component qualifies"
    );
    assert_eq!(
        snapshot.coverage().incomplete_fields,
        [0],
        "the unknown pointer target prevents proving absence on its owner",
    );
}
