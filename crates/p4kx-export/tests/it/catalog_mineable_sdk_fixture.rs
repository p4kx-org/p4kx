// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const DUPLICATE: u128 = 0x7f00_0000_0000_4000_8000_0000_0000_0002;
pub(super) const NAN_BITS: u32 = 0x7fc0_005a;

const EARLIER: u128 = DUPLICATE - 1;
const SOURCE: u128 = DUPLICATE + 1;
const ALTERNATE: u128 = DUPLICATE + 2;
const GLOBAL_PARAMS: u128 = DUPLICATE + 3;
const CONTACT: u128 = DUPLICATE + 4;
const RADAR_GLOBAL: u128 = DUPLICATE + 5;

const PROPERTY: &str = "SSCRadarContactProperites";
const TAGGED_PROPERTY: &str = "FixtureTaggedRadarProperties";
const BASE: &str = "SSCSignatureParamsBase";
const REAL_BASE: &str = "SSCSignatureSystemBaseSignatureParams";
const MALFORMED_BASE: &str = "FixtureMalformedBaseSignature";
const MODIFIER: &str = "SCSignatureTaggedEmissionModifier";
const MODIFIER_PRIORITY: &str = "fixturePriority";

#[path = "catalog_mineable_sdk_components.rs"]
mod components;
#[path = "catalog_mineable_sdk_tagged.rs"]
mod tagged;

pub(super) fn database() -> DataForgeBuilder {
    database_with_modifiers(&[])
}

fn database_with_modifiers(priorities: &[u32]) -> DataForgeBuilder {
    let database = schema(DataForgeBuilder::new().version(8));
    let full = full_record(priorities);
    let states = field_states();
    [
        entity(DUPLICATE, "FixtureFull", full),
        entity(EARLIER, "FixtureStates", states),
        entity(DUPLICATE, "FixtureBare", Vec::new()),
        record(
            DUPLICATE,
            "EntityClassDefinition",
            "FixtureOther",
            "synthetic/other",
        ),
        record(
            SOURCE,
            "MineableComposition",
            "FixtureSource",
            "libs/foundry/records/mining/rockcompositionpresets/fixture_source",
        ),
        record(
            ALTERNATE,
            "MineableComposition",
            "FixtureAlternate",
            "libs/foundry/records/mining/rockcompositionpresets/fixture_alternate",
        ),
        record(
            GLOBAL_PARAMS,
            "MiningGlobalParams",
            "FixtureParams",
            "libs/foundry/records/mining/fixture_params",
        ),
        record(
            CONTACT,
            "RadarContactTypeEntry",
            "FixtureContact",
            "synthetic/contact",
        ),
        radar_global(),
    ]
    .into_iter()
    .fold(database, DataForgeBuilder::record)
}

fn schema(database: DataForgeBuilder) -> DataForgeBuilder {
    let component = "DataForgeComponentParams";
    [
        TestStruct::new(component),
        TestStruct::new("EntityClassDefinition").array("Components", pointer(component)),
        TestStruct::new("MineableParams")
            .parent(component)
            .field(
                "composition",
                TestType::ReferenceTo("MineableComposition".to_owned()),
            )
            .field(
                "globalParams",
                TestType::ReferenceTo("MiningGlobalParams".to_owned()),
            )
            .field("fixtureCalibration", TestType::Int64),
        TestStruct::new("MineableComposition"),
        TestStruct::new("MiningGlobalParams"),
        TestStruct::new("RadarContactTypeEntry"),
        TestStruct::new(BASE),
        TestStruct::new(REAL_BASE)
            .parent(BASE)
            .array("signatures", TestType::Single),
        TestStruct::new(MALFORMED_BASE)
            .parent(BASE)
            .field("signatures", TestType::Bool),
        TestStruct::new(PROPERTY)
            .field(
                "contactType",
                TestType::ReferenceTo("RadarContactTypeEntry".to_owned()),
            )
            .field("baseSignatureParams", pointer(BASE)),
        TestStruct::new(TAGGED_PROPERTY)
            .parent(PROPERTY)
            .array("taggedEmissionModifierParams", class(MODIFIER)),
        TestStruct::new(MODIFIER).field(MODIFIER_PRIORITY, TestType::UInt32),
        TestStruct::new("SSCSignatureSystemParams")
            .parent(component)
            .field("radarProperties", pointer(PROPERTY)),
        TestStruct::new("SignatureTypeGlobalParams").field("displayName", TestType::Locale),
        TestStruct::new("SignatureSystemGlobalParams")
            .array("signatureTypeParams", class("SignatureTypeGlobalParams")),
        TestStruct::new("RadarSystemGlobalParams").field(
            "signatureSystemParams",
            class("SignatureSystemGlobalParams"),
        ),
    ]
    .into_iter()
    .fold(database, DataForgeBuilder::structure)
}

fn full_record(priorities: &[u32]) -> Vec<TestValue> {
    let signatures = [f32::from_bits(NAN_BITS), -0.0, 4270.0, 12.0]
        .into_iter()
        .map(TestValue::Single)
        .collect();
    vec![
        mining(SOURCE),
        TestValue::Null,
        signature(
            TestInstance::new(REAL_BASE).value("signatures", TestValue::Array(signatures)),
            Some(priorities),
        ),
        mining(ALTERNATE),
        mining(SOURCE),
    ]
}

fn field_states() -> Vec<TestValue> {
    let mut bad_range = 1_u32.to_le_bytes().to_vec();
    bad_range.extend_from_slice(&(u32::MAX - 1).to_le_bytes());
    [
        TestInstance::new(BASE),
        TestInstance::new(REAL_BASE).value("signatures", TestValue::Array(Vec::new())),
        TestInstance::new(MALFORMED_BASE).value("signatures", TestValue::Bool(true)),
        TestInstance::new(REAL_BASE).value("signatures", TestValue::Raw(bad_range)),
    ]
    .into_iter()
    .map(|base| signature(base, None))
    .collect()
}

fn mining(composition: u128) -> TestValue {
    TestValue::Struct(
        TestInstance::new("MineableParams")
            .value("composition", TestValue::Reference(composition))
            .value("globalParams", TestValue::Reference(GLOBAL_PARAMS))
            .value(
                "fixtureCalibration",
                TestValue::Int64(9_007_199_254_740_993),
            ),
    )
}

fn signature(base: TestInstance, priorities: Option<&[u32]>) -> TestValue {
    let properties = priorities.map_or_else(
        || TestInstance::new(PROPERTY),
        |priorities| {
            let modifiers = priorities
                .iter()
                .map(|&priority| {
                    TestValue::Struct(
                        TestInstance::new(MODIFIER)
                            .value(MODIFIER_PRIORITY, TestValue::UInt32(priority)),
                    )
                })
                .collect();
            TestInstance::new(TAGGED_PROPERTY)
                .value("taggedEmissionModifierParams", TestValue::Array(modifiers))
        },
    );
    let properties = properties
        .value("contactType", TestValue::Reference(CONTACT))
        .value("baseSignatureParams", TestValue::Struct(base));
    TestValue::Struct(
        TestInstance::new("SSCSignatureSystemParams")
            .value("radarProperties", TestValue::Struct(properties)),
    )
}

fn radar_global() -> TestRecord {
    let types = [
        "@fixture_other_one",
        "@fixture_other_two",
        "@hud_scanning_info_rs_signature",
        "@fixture_other_four",
    ]
    .into_iter()
    .map(|key| {
        TestValue::Struct(
            TestInstance::new("SignatureTypeGlobalParams")
                .value("displayName", TestValue::text(key)),
        )
    })
    .collect();
    let system = TestInstance::new("SignatureSystemGlobalParams")
        .value("signatureTypeParams", TestValue::Array(types));
    record(
        RADAR_GLOBAL,
        "RadarSystemGlobalParams",
        "FixtureRadar",
        "synthetic/radar",
    )
    .value("signatureSystemParams", TestValue::Struct(system))
}

fn record(guid: u128, ty: &str, name: &str, path: &str) -> TestRecord {
    TestRecord::new(guid, ty, format!("{ty}.{name}"), format!("{path}.xml"))
}

fn entity(guid: u128, name: &str, components: Vec<TestValue>) -> TestRecord {
    record(
        guid,
        "EntityClassDefinition",
        name,
        &format!("libs/foundry/records/entities/mineable/{name}"),
    )
    .value("Components", TestValue::Array(components))
}

fn pointer(ty: &str) -> TestType {
    TestType::StrongPointer(ty.to_owned())
}

fn class(ty: &str) -> TestType {
    TestType::Class(ty.to_owned())
}
