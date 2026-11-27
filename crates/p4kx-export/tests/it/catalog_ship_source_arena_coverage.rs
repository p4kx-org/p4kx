// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    self, ShipManufacturerValue, ShipSource, ShipSourceSnapshot,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, SourceArenaOutput, SourcePayload,
};
use p4kx_extraction::{Archive, DataForgeEntry, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use crate::commodity_v3_fixture::TestResult;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    VehicleManufacturer,
    AttachmentManufacturer,
    Insurance,
    Params,
    UnusedRoot,
}

const INVALID_BOOL_BYTE: u8 = 2;
const TRUE_BOOL_BYTE: u8 = 1;

#[test]
fn emitted_ship_source_arenas_retain_unreadable_state_and_incomplete_coverage() -> TestResult {
    for scope in [
        Scope::VehicleManufacturer,
        Scope::AttachmentManufacturer,
        Scope::Insurance,
        Scope::Params,
    ] {
        for invalid in [false, true] {
            check(scope, invalid)?;
        }
    }
    Ok(())
}

#[test]
fn unreturned_root_error_does_not_taint_fixed_ship_source_coverage() -> TestResult {
    check(Scope::UnusedRoot, true)
}

fn check(scope: Scope, invalid: bool) -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            database(scope, invalid),
        ))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let tree = forge.record(0).ok_or("original source owner")?.tree()?;
    assert_eq!(
        tree.error_count(),
        usize::from(invalid),
        "the source bool byte creates exactly the nominated decode failure"
    );
    equipment::with_ship_sources_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(0),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| coverage(&snapshot, scope, invalid),
    )??;
    Ok(())
}

fn coverage(snapshot: &ShipSourceSnapshot<'_>, scope: Scope, invalid: bool) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("one selected physical Ship".into());
    };
    let incomplete = invalid && scope != Scope::UnusedRoot;
    assert!(
        snapshot.coverage().unreadable_bodies.is_empty(),
        "the owner body is materialized with retained leaf errors"
    );
    assert_eq!(
        snapshot.coverage().incomplete_fields.as_slice(),
        if incomplete { &[0][..] } else { &[] },
        "owner coverage follows the emitted arena cells only"
    );
    assert_eq!(
        snapshot.dependency_coverage().is_complete(),
        !incomplete,
        "returned unavailable source cells prevent complete coverage"
    );
    assert_eq!(
        snapshot
            .dependency_coverage()
            .incomplete_records()
            .contains(&0),
        incomplete,
        "the physical source owner carries the incomplete field proof"
    );
    assert!(
        snapshot.dependency_coverage().failed_entries().is_empty(),
        "no Entry operation was required for an empty vehicle selector"
    );
    if scope == Scope::UnusedRoot {
        return Ok(());
    }
    retained_node(arena(row, scope)?, invalid)
}

fn retained_node(arena: &SourceArenaOutput<'_>, invalid: bool) -> TestResult {
    let node = arena
        .values()
        .iter()
        .find(|node| {
            node.original_error().is_some()
                || matches!(node.value().value(), Some(SourcePayload::Boolean { .. }))
        })
        .ok_or("one authored additional Bool occurrence")?;
    assert_eq!(
        node.value().state(),
        if invalid {
            SourceValueState::Unreadable
        } else {
            SourceValueState::Present
        },
        "the actual decoder state stays visible in the returned arena"
    );
    assert_eq!(
        node.original_error().is_some(),
        invalid,
        "unreadable source retains the original diagnostic facts"
    );
    assert_eq!(
        node.value().origin().source_set_complete(),
        !invalid,
        "only the readable field has complete source evidence"
    );
    Ok(())
}

fn arena<'row, 'source>(
    row: &'row ShipSource<'source>,
    scope: Scope,
) -> TestResult<&'row SourceArenaOutput<'source>> {
    match scope {
        Scope::VehicleManufacturer => {
            manufacturer(row.vehicle_manufacturer().manufacturer().value())
        }
        Scope::AttachmentManufacturer => {
            manufacturer(row.attachment_manufacturer().manufacturer().value())
        }
        Scope::Insurance => Ok(row
            .insurance()
            .value()
            .ok_or("original insurance")?
            .additional_fields()),
        Scope::Params => Ok(row
            .insurance()
            .value()
            .ok_or("original insurance")?
            .params()
            .value()
            .ok_or("original parameters")?
            .additional_fields()),
        Scope::UnusedRoot => Err("unreturned fields have no ShipSource arena".into()),
    }
}

fn manufacturer<'row, 'source>(
    value: Option<&'row ShipManufacturerValue<'source>>,
) -> TestResult<&'row SourceArenaOutput<'source>> {
    match value {
        Some(ShipManufacturerValue::Authored { authored }) => Ok(authored),
        _ => Err("original Class manufacturer must remain authored".into()),
    }
}

fn database(scope: Scope, invalid: bool) -> Vec<u8> {
    let record = TestRecord::new(
        0x9191,
        "EntityClassDefinition",
        "EntityClassDefinition.CoverageShip",
        "libs/foundry/records/entities/spaceships/coverage.xml",
    )
    .value(
        "Components",
        TestValue::Array(component_values(scope, invalid)),
    )
    .value(
        "StaticEntityClassData",
        TestValue::Array(vec![TestValue::Struct(insurance_value(scope, invalid))]),
    )
    .value("unused", flag(scope, Scope::UnusedRoot, invalid));
    declarations().record(record).build()
}

fn component_values(scope: Scope, invalid: bool) -> Vec<TestValue> {
    let extra = |at| TestInstance::new("Extra").value("flag", flag(scope, at, invalid));
    let vehicle = TestInstance::new("VehicleComponentParams")
        .value("vehicleDefinition", TestValue::text(""))
        .value("modification", TestValue::text(""))
        .value(
            "manufacturer",
            TestValue::Struct(extra(Scope::VehicleManufacturer)),
        );
    let attachment = TestInstance::new("SAttachableComponentParams").value(
        "AttachDef",
        TestValue::Struct(
            TestInstance::new("AttachDef")
                .value("Type", TestValue::text("NOITEM_Vehicle"))
                .value(
                    "Manufacturer",
                    TestValue::Struct(extra(Scope::AttachmentManufacturer)),
                ),
        ),
    );
    vec![TestValue::Struct(vehicle), TestValue::Struct(attachment)]
}

fn insurance_value(scope: Scope, invalid: bool) -> TestInstance {
    let params = TestInstance::new("ShipInsuranceParams")
        .value("shipEntityClassName", TestValue::text("SyntheticShip"))
        .value("baseWaitTimeMinutes", TestValue::Single(1.0))
        .value("mandatoryWaitTimeMinutes", TestValue::Single(0.0))
        .value("baseExpeditingFee", TestValue::UInt32(0))
        .value("flag", flag(scope, Scope::Params, invalid));
    TestInstance::new("SEntityInsuranceProperties")
        .value("restoreOnClaim", TestValue::Bool(true))
        .value("shipInsuranceParams", TestValue::Struct(params))
        .value("flag", flag(scope, Scope::Insurance, invalid))
}

fn flag(scope: Scope, at: Scope, invalid: bool) -> TestValue {
    TestValue::Raw(vec![if scope == at && invalid {
        INVALID_BOOL_BYTE
    } else {
        TRUE_BOOL_BYTE
    }])
}

fn declarations() -> DataForgeBuilder {
    let source = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Component"))
        .structure(TestStruct::new("Extra").field("flag", TestType::Bool))
        .structure(TestStruct::new("StaticEntityClassData"));
    insurance_schema(component_schema(source)).structure(
        TestStruct::new("EntityClassDefinition")
            .array(
                "Components",
                TestType::StrongPointer("Component".to_owned()),
            )
            .array(
                "StaticEntityClassData",
                TestType::StrongPointer("StaticEntityClassData".to_owned()),
            )
            .field("unused", TestType::Bool),
    )
}

fn component_schema(source: DataForgeBuilder) -> DataForgeBuilder {
    source
        .structure(
            TestStruct::new("VehicleComponentParams")
                .parent("Component")
                .field("vehicleDefinition", TestType::String)
                .field("modification", TestType::String)
                .field("manufacturer", TestType::Class("Extra".to_owned())),
        )
        .structure(
            TestStruct::new("AttachDef")
                .field("Type", TestType::String)
                .field("Manufacturer", TestType::Class("Extra".to_owned())),
        )
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("Component")
                .field("AttachDef", TestType::Class("AttachDef".to_owned())),
        )
}

fn insurance_schema(source: DataForgeBuilder) -> DataForgeBuilder {
    source
        .structure(
            TestStruct::new("ShipInsuranceParams")
                .field("shipEntityClassName", TestType::String)
                .field("baseWaitTimeMinutes", TestType::Single)
                .field("mandatoryWaitTimeMinutes", TestType::Single)
                .field("baseExpeditingFee", TestType::UInt32)
                .field("flag", TestType::Bool),
        )
        .structure(
            TestStruct::new("SEntityInsuranceProperties")
                .parent("StaticEntityClassData")
                .field("restoreOnClaim", TestType::Bool)
                .field(
                    "shipInsuranceParams",
                    TestType::Class("ShipInsuranceParams".to_owned()),
                )
                .field("flag", TestType::Bool),
        )
}
