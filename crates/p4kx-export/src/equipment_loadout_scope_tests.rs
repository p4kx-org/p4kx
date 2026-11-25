// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{LoadoutOwnerSelection, read, read_visit};
use crate::Kind;
use crate::catalog_queries::CatalogRecordSelection;
use crate::entity_selection::{self, Selected};
use crate::loadout;
use crate::loadout::PortAdmission;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ENTITY: &str = p4kx_assembly::ENTITY_TYPE;
const CONTAINER: &str = "SItemPortContainerComponentParams";

#[test]
fn malformed_loadout_selection_shapes_report_incomplete_without_decoder_errors() -> TestResult {
    let components = DataForgeBuilder::new()
        .structure(TestStruct::new(ENTITY).field("Components", TestType::Bool))
        .record(owner().value("Components", TestValue::Bool(true)));
    let ports = DataForgeBuilder::new()
        .structure(
            TestStruct::new(ENTITY)
                .array("Components", TestType::StrongPointer(CONTAINER.to_owned())),
        )
        .structure(TestStruct::new(CONTAINER).field("Ports", TestType::Bool))
        .record(owner().value(
            "Components",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new(CONTAINER).value("Ports", TestValue::Bool(true)),
            )]),
        ));
    for database in [components, ports] {
        assert_scope(&database.build(), 0, &[0])?;
    }
    Ok(())
}

#[test]
fn unrelated_original_decoder_error_preserves_proved_loadout_absence() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new(ENTITY).field("FixtureUnrelated", TestType::Bool))
        .record(owner().value("FixtureUnrelated", TestValue::Raw(vec![2])))
        .build();
    assert_scope(&bytes, 1, &[])
}

#[test]
fn ship_roots_skip_standalone_item_admission_fields() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new(ENTITY).field("Components", TestType::Bool))
        .record(owner().value("Components", TestValue::Bool(true)))
        .record(TestRecord::new(
            2,
            ENTITY,
            "EntityClassDefinition.LoadoutShip",
            "libs/foundry/records/entities/spaceships/loadout_ship.xml",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let all = read(&forge, CatalogRecordSelection::All, None)?;
    assert_eq!(
        all.coverage.incomplete_fields,
        [0],
        "the standalone Item has malformed admission fields"
    );
    let ships = read_visit(&forge, LoadoutOwnerSelection::Ships, None)?;
    let [ship] = ships.owners.wanted.as_slice() else {
        return Err("one admitted Ship".into());
    };
    assert_eq!(ship.record_index, 1, "the Ship remains an admitted root");
    assert!(
        ships.owners.coverage.incomplete_fields.is_empty(),
        "Item admission is outside this selection"
    );
    assert!(
        ships.missing_record_indices.is_empty(),
        "only explicit batches report absent indices"
    );
    Ok(())
}

fn assert_scope(bytes: &[u8], errors: usize, incomplete: &[u32]) -> TestResult {
    let forge = DataForge::open(bytes)?;
    let record = forge.record(0).ok_or("original physical fixture Item")?;
    assert_eq!(
        entity_selection::select(&record, None)?,
        Some(Selected::Entity(Kind::Items)),
        "the original header admits this Item",
    );
    let tree = record.tree()?;
    assert_eq!(tree.error_count(), errors, "original decoder control");
    assert!(
        !loadout::carries_ports(&tree),
        "producer still omits this Item"
    );
    let admission = PortAdmission::read(&tree);
    assert!(!admission.wanted, "the query keeps the producer membership");
    let scope = read(&forge, CatalogRecordSelection::RecordIndex(0), None)?;
    assert!(scope.wanted.is_empty());
    assert_eq!(
        scope.coverage.incomplete_fields, incomplete,
        "scope covers only original loadout selection inputs"
    );
    assert_eq!(
        scope.coverage.unreadable_bodies,
        [] as [u32; 0],
        "the body remains readable even when a selection field has the wrong shape",
    );
    Ok(())
}

fn owner() -> TestRecord {
    TestRecord::new(
        1,
        ENTITY,
        "EntityClassDefinition.LoadoutAdmission",
        "libs/foundry/records/entities/scitem/fixture/admission.xml",
    )
}
