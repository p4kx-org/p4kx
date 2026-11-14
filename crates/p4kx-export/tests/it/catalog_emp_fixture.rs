// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const EMP: &str = "SCItemEMPParams";
pub(super) const PARTICLE: &str = "GlobalResourceParticle";
pub(super) const STATE: &str = "ItemResourceState";
pub(super) const TAG: &str = "Tag";
pub(super) const OWNER: u128 = 0x1077;
pub(super) const CHARGED_TAG: u128 = 0x1080;
pub(super) const CHARGING_TAG: u128 = 0x1081;
pub(super) const FIRST_TRIGGER: u128 = 0x1090;
pub(super) const MISSING_TRIGGER: u128 = 0x10ff;
pub(super) const PRESSURE_NAN: u32 = 0x7fc0_1077;
pub(super) const PARAMETERS: [&str; 9] = [
    "chargeTime",
    "unleashTime",
    "cooldownTime",
    "empRadius",
    "minEmpRadius",
    "physRadius",
    "minPhysRadius",
    "distortionDamage",
    "pressure",
];
pub(super) const TRIGGERS: [(&str, &str); 6] = [
    ("StartChargingTrigger", "startChargingTrigger"),
    ("StopChargingTrigger", "stopChargingTrigger"),
    ("StartChargedTrigger", "startChargedTrigger"),
    ("StopChargedTrigger", "stopChargedTrigger"),
    ("StartUnleashTrigger", "startUnleashTrigger"),
    ("StopUnleashTrigger", "stopUnleashTrigger"),
];

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Full,
    Reduced,
    WrongDeclaration,
    NoEmp,
}

pub(super) fn database(layout: Layout) -> DataForgeBuilder {
    let mut forge = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", pointer("DataForgeComponentParams")),
        )
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", class("FixtureEmpAttachment")),
        )
        .structure(TestStruct::new("FixtureEmpAttachment").field("Type", TestType::String))
        .structure(TestStruct::new(STATE).field("StateName", TestType::String))
        .structure(
            TestStruct::new(PARTICLE)
                .field("path", TestType::String)
                .field("fixtureVariation", TestType::UInt16),
        )
        .structure(TestStruct::new(TAG).field("tagName", TestType::String));
    if !matches!(layout, Layout::NoEmp) {
        forge = forge.structure(emp_schema(layout));
    }
    forge = forge.record(owner(layout));
    for (guid, name) in [(CHARGED_TAG, "Charged"), (CHARGING_TAG, "Charging")] {
        forge = forge.record(
            TestRecord::new(guid, TAG, name, format!("synthetic/emp/{name}.xml"))
                .value("tagName", TestValue::text(name)),
        );
    }
    for (index, (name, _)) in (0_u128..).zip(TRIGGERS) {
        if index != 5 {
            forge = forge.record(
                TestRecord::new(
                    FIRST_TRIGGER + index,
                    TAG,
                    name,
                    format!("synthetic/emp/{name}.xml"),
                )
                .value("tagName", TestValue::text(name)),
            );
        }
    }
    forge
}

fn emp_schema(layout: Layout) -> TestStruct {
    let mut schema = TestStruct::new(EMP)
        .parent("DataForgeComponentParams")
        .field("fixtureOwnedState", pointer(STATE))
        .field("fixtureProfile", TestType::UInt16);
    for name in PARAMETERS {
        if name == "cooldownTime" && matches!(layout, Layout::Reduced) {
            continue;
        }
        let kind = if name == "pressure" && matches!(layout, Layout::WrongDeclaration) {
            TestType::UInt16
        } else {
            TestType::Single
        };
        schema = schema.field(name, kind);
    }
    for name in [
        "idleState",
        "chargingState",
        "chargedState",
        "releasingState",
    ] {
        schema = schema.field(name, TestType::WeakPointer(STATE.to_owned()));
    }
    schema = schema
        .field("ChargedParticle", class(PARTICLE))
        .field("ChargingParticle", class(PARTICLE))
        .field("ChargedTag", TestType::ReferenceTo(TAG.to_owned()))
        .field("ChargingTag", TestType::ReferenceTo(TAG.to_owned()));
    for (name, _) in TRIGGERS {
        schema = schema.field(name, TestType::ReferenceTo(TAG.to_owned()));
    }
    schema
}

fn owner(layout: Layout) -> TestRecord {
    let attach = TestInstance::new("SAttachableComponentParams").value(
        "AttachDef",
        TestValue::Struct(
            TestInstance::new("FixtureEmpAttachment").value("Type", TestValue::text("Misc")),
        ),
    );
    let mut values = vec![TestValue::Struct(attach)];
    if !matches!(layout, Layout::NoEmp) {
        values.push(TestValue::Struct(emp(layout, "fixture-first", 4)));
    }
    if matches!(layout, Layout::Full) {
        values.push(TestValue::Struct(emp(layout, "fixture-second", 20)));
    }
    TestRecord::new(
        OWNER,
        "EntityClassDefinition",
        "EntityClassDefinition.fixture_emp",
        "libs/foundry/records/entities/scitem/ships/fixture_emp.xml",
    )
    .value("Components", TestValue::Array(values))
}

fn emp(layout: Layout, state_name: &str, seed: u16) -> TestInstance {
    let mut component = TestInstance::new(EMP)
        .value(
            "fixtureOwnedState",
            TestValue::Struct(
                TestInstance::new(STATE)
                    .named(state_name)
                    .value("StateName", TestValue::text(state_name)),
            ),
        )
        .value("fixtureProfile", TestValue::UInt16(seed))
        .value("idleState", TestValue::pointer(state_name))
        .value("chargingState", TestValue::Null)
        .value("chargedState", TestValue::pointer(state_name))
        .value("releasingState", TestValue::Null);
    for (index, name) in (0_u16..).zip(PARAMETERS) {
        if name == "cooldownTime" && matches!(layout, Layout::Reduced) {
            continue;
        }
        let value = match name {
            "pressure" if matches!(layout, Layout::WrongDeclaration) => TestValue::UInt16(42),
            "pressure" => TestValue::Single(f32::from_bits(PRESSURE_NAN)),
            "minPhysRadius" => TestValue::Single(-0.0),
            _ => TestValue::Single(f32::from(seed + index)),
        };
        component = component.value(name, value);
    }
    with_presentation(component, seed)
}

fn with_presentation(mut component: TestInstance, seed: u16) -> TestInstance {
    for (name, path) in [
        ("ChargedParticle", "fixture/charged"),
        ("ChargingParticle", ""),
    ] {
        component = component.value(
            name,
            TestValue::Struct(
                TestInstance::new(PARTICLE)
                    .value("path", TestValue::text(path))
                    .value("fixtureVariation", TestValue::UInt16(seed)),
            ),
        );
    }
    component = component
        .value("ChargedTag", TestValue::Reference(CHARGED_TAG))
        .value("ChargingTag", TestValue::Reference(CHARGING_TAG));
    for (index, (name, _)) in (0_u128..).zip(TRIGGERS) {
        let value = match index {
            4 => TestValue::Null,
            5 => TestValue::Reference(MISSING_TRIGGER),
            _ => TestValue::Reference(FIRST_TRIGGER + index),
        };
        component = component.value(name, value);
    }
    component
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
