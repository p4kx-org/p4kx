// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const OBJECT: &str = "StarMapObject";
const TYPE: &str = "StarMapObjectType";
const AMENITY: &str = "StarMapAmenityTypeEntry";
const JURISDICTION: &str = "Jurisdiction";
const LEGACY_FACTION: &str = "Faction_LEGACY";
const TRAVEL: &str = "StarMapQuantumTravelDataParams";
const CHILD: u128 = 0x1177_9150;
const OBJECT_TYPE: u128 = 0x1177_9151;
const MEDICAL: u128 = 0x1177_9152;
const LAW: u128 = 0x1177_9153;
const AFFILIATION: u128 = 0x1177_9154;
const PARENT: u128 = 0x1177_9155;

pub(super) fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(object_schema())
        .structure(TestStruct::new(TYPE).field("name", TestType::String))
        .structure(TestStruct::new(AMENITY).field("name", TestType::String))
        .structure(TestStruct::new(JURISDICTION))
        .structure(TestStruct::new(LEGACY_FACTION))
        .structure(
            TestStruct::new(TRAVEL)
                .field("obstructionRadius", TestType::Single)
                .field("arrivalRadius", TestType::Single)
                .field("adoptionRadius", TestType::Single)
                .field("arrivalPointDetectionOffset", TestType::Single),
        )
        .record(child())
        .record(named(OBJECT_TYPE, TYPE, "FixtureType").value("name", TestValue::text("Type")))
        .record(named(MEDICAL, AMENITY, "FixtureMedical").value("name", TestValue::text("Medical")))
        .record(named(LAW, JURISDICTION, "FixtureLaw"))
        .record(named(AFFILIATION, LEGACY_FACTION, "FixtureAffiliation"))
        .record(
            object(PARENT, "FixtureParent")
                .value("name", TestValue::text("@fixture_object_parent"))
                .value("size", TestValue::Double(f64::NEG_INFINITY))
                .value("quantumTravelData", TestValue::Null),
        )
}

fn object_schema() -> TestStruct {
    TestStruct::new(OBJECT)
        .field("name", TestType::Locale)
        .field("description", TestType::Locale)
        .field("callout1", TestType::Locale)
        .field("size", TestType::Double)
        .field("minimumDisplaySize", TestType::Single)
        .field("type", reference(TYPE))
        .field("parent", reference(OBJECT))
        .array("amenities", reference(AMENITY))
        .field("jurisdiction", reference(JURISDICTION))
        .field("affiliation", reference(LEGACY_FACTION))
        .field("hideInStarmap", TestType::Bool)
        .field("hideInWorld", TestType::Bool)
        .field("blockTravel", TestType::Bool)
        .field(
            "quantumTravelData",
            TestType::StrongPointer(TRAVEL.to_owned()),
        )
}

fn child() -> TestRecord {
    object(CHILD, "FixtureChild")
        .value("name", TestValue::text("@fixture_object_child"))
        .value(
            "description",
            TestValue::text("@fixture_object_description"),
        )
        .value(
            "callout1",
            TestValue::text("@fixture_unused_object_callout"),
        )
        .value("size", TestValue::Double(-0.0))
        .value("minimumDisplaySize", TestValue::Single(10.0))
        .value("type", TestValue::Reference(OBJECT_TYPE))
        .value("parent", TestValue::Reference(PARENT))
        .value(
            "amenities",
            TestValue::Array(vec![
                TestValue::Reference(MEDICAL),
                TestValue::Null,
                TestValue::Reference(MEDICAL),
            ]),
        )
        .value("jurisdiction", TestValue::Reference(LAW))
        .value("affiliation", TestValue::Reference(AFFILIATION))
        .value("hideInStarmap", TestValue::Bool(true))
        .value("hideInWorld", TestValue::Bool(false))
        .value("blockTravel", TestValue::Bool(true))
        .value(
            "quantumTravelData",
            TestValue::Struct(
                TestInstance::new(TRAVEL)
                    .value("obstructionRadius", TestValue::Single(20.5))
                    .value("arrivalRadius", TestValue::Single(5.0))
                    .value("adoptionRadius", TestValue::Single(f32::INFINITY))
                    .value("arrivalPointDetectionOffset", TestValue::Single(0.125)),
            ),
        )
}

fn object(id: u128, name: &str) -> TestRecord {
    TestRecord::new(
        id,
        OBJECT,
        format!("{OBJECT}.{name}"),
        format!("libs/foundry/records/starmap/pu/system/{name}.xml"),
    )
}

fn named(id: u128, kind: &str, name: &str) -> TestRecord {
    TestRecord::new(
        id,
        kind,
        format!("{kind}.{name}"),
        format!("fixtures/{name}.xml"),
    )
}

fn reference(kind: &str) -> TestType {
    TestType::ReferenceTo(kind.to_owned())
}
