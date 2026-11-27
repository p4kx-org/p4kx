// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForge, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use crate::build::original_input::test_support;
use crate::catalog_projection::CatalogProjection;
use crate::texts::TextSources;

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;
pub(super) const VEHICLE: &str = "VehicleComponentParams";
pub(super) const INSURANCE: &str = "SEntityInsuranceProperties";
pub(super) const OWNER: u128 = 0x1166_0000_0000_4000_8000_0000_0000_b201;
pub(super) const VARIANT_BASE_INDEX: u32 = 1;
pub(super) const CREW: u128 = 0x1166_0000_0000_4000_8000_0000_0000_b202;
const BAD_INSTANCE: u32 = u32::MAX - 1;
const VEHICLE_DECLARATION: u32 = 2;
const INSURANCE_DECLARATION: u32 = 6;
const PARAMS_DECLARATION: u32 = 7;
pub(super) const NAN_BITS: u64 = 0x7ff8_0000_0000_00b2;

#[derive(Clone, Copy)]
pub(super) enum Origin {
    Plain,
    Encrypted,
    Unknown,
}

#[derive(Clone, Copy)]
pub(super) enum Fault {
    None,
    EarlierComponent,
    EarlierStatic,
    LaterStatic,
    NullParams,
    UnreadableParams,
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

pub(super) fn database(fault: Fault) -> DataForgeBuilder {
    declarations()
        .record(owner(
            "Primary",
            fault,
            4,
            "  Data\\Vehicles\\Original.xml  ",
        ))
        .record(owner("Alias", Fault::None, 9, ""))
        .record(TestRecord::new(
            CREW,
            "CrewManifest",
            "CrewManifest.Broken",
            "libs/foundry/records/crew/broken.xml",
        ))
}

pub(super) fn without_vehicle(failed: bool) -> DataForgeBuilder {
    let values = if failed {
        vec![bad_pointer(VEHICLE_DECLARATION)]
    } else {
        Vec::new()
    };
    declarations().record(
        TestRecord::new(
            OWNER,
            "EntityClassDefinition",
            "EntityClassDefinition.WithoutVehicle",
            "libs/foundry/records/entities/spaceships/without_vehicle.xml",
        )
        .value("Components", TestValue::Array(values)),
    )
}

fn declarations() -> DataForgeBuilder {
    [
        TestStruct::new("Component"),
        TestStruct::new("Static"),
        TestStruct::new(VEHICLE)
            .parent("Component")
            .field("crewSize", TestType::UInt64)
            .field("vehicleDefinition", TestType::String)
            .field("vehicleName", TestType::Locale)
            .field("vehicleDescription", TestType::Locale)
            .field("vehicleRole", TestType::Locale)
            .field("vehicleCareer", TestType::Locale),
        TestStruct::new("SAttachableComponentParams")
            .parent("Component")
            .field("AttachDef", TestType::Class("ForeignAttachDef".to_owned())),
        TestStruct::new("ForeignAttachDef").field("Size", TestType::UInt64),
        TestStruct::new("SEntityComponentCrewManifestParams")
            .parent("Component")
            .field(
                "crewManifest",
                TestType::ReferenceTo("CrewManifest".to_owned()),
            ),
        TestStruct::new(INSURANCE)
            .parent("Static")
            .field("restoreOnClaim", TestType::Bool)
            .field("shipInsuranceParams", pointer("ShipInsuranceParams")),
        TestStruct::new("ShipInsuranceParams")
            .field("shipEntityClassName", TestType::String)
            .field("baseWaitTimeMinutes", TestType::Double)
            .field("mandatoryWaitTimeMinutes", TestType::Single)
            .field("baseExpeditingFee", TestType::UInt64)
            .field("futureNumber", TestType::Double),
        TestStruct::new("EntityClassDefinition")
            .array("Components", pointer("Component"))
            .array("StaticEntityClassData", pointer("Static")),
        TestStruct::new("CrewManifest")
            .field("bad", TestType::UInt64)
            .declared_size(1),
        TestStruct::new("EAEntityDataParams")
            .parent("Static")
            .field("inclusionMode", TestType::String),
    ]
    .into_iter()
    .fold(
        DataForgeBuilder::new().version(8),
        DataForgeBuilder::structure,
    )
}

fn bad_pointer(structure: u32) -> TestValue {
    let mut bytes = structure.to_le_bytes().to_vec();
    bytes.extend_from_slice(&BAD_INSTANCE.to_le_bytes());
    TestValue::Raw(bytes)
}

fn owner(name: &str, fault: Fault, crew: u64, definition: &str) -> TestRecord {
    let vehicle = TestValue::Struct(
        TestInstance::new(VEHICLE)
            .value("crewSize", TestValue::UInt64(crew))
            .value("vehicleDefinition", TestValue::text(definition))
            .value("vehicleName", TestValue::text("@ship_name")),
    );
    let attachment = TestValue::Struct(TestInstance::new("SAttachableComponentParams").value(
        "AttachDef",
        TestValue::Struct(
            TestInstance::new("ForeignAttachDef").value("Size", TestValue::UInt64(u64::MAX)),
        ),
    ));
    let manifest = TestValue::Struct(
        TestInstance::new("SEntityComponentCrewManifestParams")
            .value("crewManifest", TestValue::Reference(CREW)),
    );
    let mut components = vec![TestValue::Null];
    if matches!(fault, Fault::EarlierComponent) {
        components.push(bad_pointer(VEHICLE_DECLARATION));
    }
    components.extend([vehicle, attachment, manifest]);
    let params = if matches!(fault, Fault::NullParams) {
        TestValue::Null
    } else if matches!(fault, Fault::UnreadableParams) {
        bad_pointer(PARAMS_DECLARATION)
    } else {
        TestValue::Struct(
            TestInstance::new("ShipInsuranceParams")
                .value("shipEntityClassName", TestValue::text(""))
                .value("baseWaitTimeMinutes", TestValue::Double(0.0))
                .value("mandatoryWaitTimeMinutes", TestValue::Single(-0.0))
                .value("baseExpeditingFee", TestValue::UInt64(u64::MAX))
                .value("futureNumber", TestValue::Double(f64::from_bits(NAN_BITS))),
        )
    };
    let insurance = TestValue::Struct(
        TestInstance::new(INSURANCE)
            .value("restoreOnClaim", TestValue::Bool(false))
            .value("shipInsuranceParams", params),
    );
    let mut data = vec![TestValue::Null];
    if matches!(fault, Fault::EarlierStatic) {
        data.push(bad_pointer(INSURANCE_DECLARATION));
    }
    data.push(insurance);
    data.push(TestValue::Struct(
        TestInstance::new(INSURANCE).value("restoreOnClaim", TestValue::Bool(true)),
    ));
    if matches!(fault, Fault::LaterStatic) {
        data.push(bad_pointer(INSURANCE_DECLARATION));
    }
    TestRecord::new(
        OWNER,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{name}"),
        "libs/foundry/records/entities/spaceships/source.xml",
    )
    .value("Components", TestValue::Array(components))
    .value("StaticEntityClassData", TestValue::Array(data))
}

pub(super) fn with_projection(
    database: &DataForgeBuilder,
    origin: Origin,
    inspect: impl for<'source, 'context> FnOnce(
        &'source DataForge<'source>,
        &CatalogProjection<'context, 'source>,
    ) -> TestResult,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("ship-source.p4k");
    let dcb = TestEntry::stored("Data/Game2.dcb", database.build());
    let dcb = match origin {
        Origin::Plain => dcb,
        Origin::Encrypted => dcb.encrypted(),
        Origin::Unknown => dcb.encrypted().with_encryption_field(7),
    };
    ArchiveBuilder::new()
        .entry(dcb)
        .entry(
            TestEntry::stored(Language::English.global_ini_path(), b"source=unreadable\n")
                .encrypted()
                .with_payload(vec![1, 2, 3]),
        )
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    assert!(
        TextSources::read_languages(&archive, &[Language::English]).is_err(),
        "the unused English payload is genuinely unreadable"
    );
    let sources = TextSources::read_languages(&archive, &[])?;
    test_support::with_archive_projection(&archive, &sources, |forge, projection| {
        check_declarations(forge)?;
        inspect(forge, &projection.for_kind(crate::Kind::Ships))
    })
}

fn check_declarations(forge: &DataForge<'_>) -> TestResult {
    for (index, expected) in [
        (VEHICLE_DECLARATION, VEHICLE),
        (INSURANCE_DECLARATION, INSURANCE),
        (PARAMS_DECLARATION, "ShipInsuranceParams"),
    ] {
        assert_eq!(
            forge
                .structs()
                .nth(usize::try_from(index)?)
                .ok_or("original fixture declaration")?
                .name()?,
            expected,
            "bad-pointer controls retain the actual declared original family"
        );
    }
    Ok(())
}

pub(super) fn variant_candidates() -> DataForgeBuilder {
    declarations()
        .record(variant(
            "A_Selected",
            OWNER,
            "Data/Vehicles/Match.xml",
            "DoNotInclude",
        ))
        .record(variant(
            "Z_Base",
            OWNER + 1,
            "data\\vehicles\\match.xml",
            "ReadyToInclude",
        ))
        .record(variant(
            "A_Alias",
            OWNER + 1,
            "data/vehicles/other.xml",
            "ReadyToInclude",
        ))
}

fn variant(class: &str, guid: u128, definition: &str, inclusion: &str) -> TestRecord {
    let vehicle = TestInstance::new(VEHICLE)
        .value("vehicleDefinition", TestValue::text(definition))
        .value("vehicleName", TestValue::text("@ship_name"));
    let entity =
        TestInstance::new("EAEntityDataParams").value("inclusionMode", TestValue::text(inclusion));
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{class}"),
        format!("libs/foundry/records/entities/spaceships/{class}.xml"),
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(vehicle)]),
    )
    .value(
        "StaticEntityClassData",
        TestValue::Array(vec![TestValue::Struct(entity)]),
    )
}
