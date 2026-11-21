// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const DRIVE: &str = "SCItemQuantumDriveParams";
pub(super) const STANDARD: &str = "FixtureQuantumStandard";
pub(super) const SPLINE: &str = "FixtureQuantumSpline";
pub(super) const HEAT: &str = "FixtureQuantumHeat";
pub(super) const EMPTY_HEAT: &str = "FixtureQuantumEmptyHeat";
pub(super) const BOOST: &str = "FixtureQuantumBoost";
const STATE: &str = "FixtureQuantumState";
const TAG: &str = "FixtureQuantumTag";
const TAG_LINK: &str = "FixtureQuantumTagLink";
pub(super) const OWNER: u128 = 0x1055;
pub(super) const FIRST_TAG: u128 = 0x1060;
pub(super) const SECOND_TAG: u128 = 0x1061;
pub(super) const ROOT_GUID: u128 = 0x1062;
pub(super) const NONFINITE_RANGE: u64 = 0x7ff8_0000_0000_1055;

pub(super) const PROFILE_FIELDS: [(&str, &str); 24] = [
    ("driveSpeed", "driveSpeed"),
    ("cooldownTime", "cooldownTime"),
    ("spoolUpTime", "spoolUpTime"),
    ("stageOneAccelRate", "stageOneAccelRate"),
    ("stageTwoAccelRate", "stageTwoAccelRate"),
    ("engageSpeed", "engageSpeed"),
    ("calibrationRate", "calibrationRate"),
    ("minCalibrationRequirement", "minCalibrationRequirement"),
    ("maxCalibrationRequirement", "maxCalibrationRequirement"),
    (
        "calibrationProcessAngleLimit",
        "calibrationProcessAngleLimit",
    ),
    (
        "calibrationWarningAngleLimit",
        "calibrationWarningAngleLimit",
    ),
    ("calibrationDelayInSeconds", "calibrationDelayInSeconds"),
    ("interdictionEffectTime", "interdictionEffectTime"),
    ("VFXSpoolEndVelocity", "vfxSpoolEndVelocity"),
    ("VFXPinchEffectTime", "vfxPinchEffectTime"),
    ("VFXPinchMaxVelocity", "vfxPinchMaxVelocity"),
    ("VFXEntryFlashVelocity", "vfxEntryFlashVelocity"),
    ("VFXTrailStartVelocity", "vfxTrailStartVelocity"),
    (
        "VFXTravelEffectStartVelocity",
        "vfxTravelEffectStartVelocity",
    ),
    ("VFXTravelEffectEndVelocity", "vfxTravelEffectEndVelocity"),
    ("VFXExitEffectVelocity", "vfxExitEffectVelocity"),
    ("ShaderNodeEngageVelocity", "shaderNodeEngageVelocity"),
    ("ShaderNodeShutdownVelocity", "shaderNodeShutdownVelocity"),
    (
        "ShaderNodeMaxStrengthVelocity",
        "shaderNodeMaxStrengthVelocity",
    ),
];
pub(super) const HEAT_FIELDS: [(&str, &str); 5] = [
    ("preRampUpThermalEnergyDraw", "preRampUpThermalEnergyDraw"),
    ("rampUpThermalEnergyDraw", "rampUpThermalEnergyDraw"),
    ("inFlightThermalEnergyDraw", "inFlightThermalEnergyDraw"),
    ("rampDownThermalEnergyDraw", "rampDownThermalEnergyDraw"),
    (
        "postRampDownThermalEnergyDraw",
        "postRampDownThermalEnergyDraw",
    ),
];
pub(super) const BOOST_FIELDS: [(&str, &str); 8] = [
    ("maxBoostSpeed", "maxBoostSpeed"),
    ("timeToMaxBoostSpeed", "timeToMaxBoostSpeed"),
    ("boostUseTime", "boostUseTime"),
    ("boostRechargeTime", "boostRechargeTime"),
    ("stopTime", "stopTime"),
    ("minJumpDistance", "minJumpDistance"),
    ("ifcsHandoverDownTime", "ifcsHandoverDownTime"),
    ("ifcsHandoverRespoolTime", "ifcsHandoverRespoolTime"),
];

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Full,
    WithoutSpline,
    WithoutStandard,
    InvalidSpline,
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
                .field("AttachDef", class("FixtureQuantumAttach")),
        )
        .structure(TestStruct::new("FixtureQuantumAttach").field("Type", TestType::String))
        .structure(drive_schema(layout))
        .structure(profile_schema(STANDARD))
        .structure(profile_schema(SPLINE))
        .structure(number_schema(HEAT, &HEAT_FIELDS))
        .structure(TestStruct::new(EMPTY_HEAT))
        .structure(number_schema(BOOST, &BOOST_FIELDS))
        .structure(TestStruct::new(STATE).field("value", TestType::String))
        .structure(
            TestStruct::new(TAG_LINK).field("tagRecord", TestType::ReferenceTo(TAG.to_owned())),
        )
        .structure(TestStruct::new(TAG))
        .record(owner(layout));
    for (guid, name) in [(FIRST_TAG, "first"), (SECOND_TAG, "second")] {
        forge = forge.record(TestRecord::new(
            guid,
            TAG,
            name,
            format!("fixture/quantum/{name}.xml"),
        ));
    }
    forge
}

fn drive_schema(layout: Layout) -> TestStruct {
    let mut drive = TestStruct::new(DRIVE)
        .parent("DataForgeComponentParams")
        .field("quantumFuelRequirement", TestType::Single)
        .field("jumpRange", TestType::Single)
        .field("disconnectRange", TestType::Double)
        .field("fixtureRootGuid", TestType::Guid)
        .field(
            "fixtureNullReference",
            TestType::ReferenceTo(TAG.to_owned()),
        );
    if !matches!(layout, Layout::WithoutStandard) {
        drive = drive.field("params", class(STANDARD));
    }
    match layout {
        Layout::Full => drive
            .field("splineJumpParams", class(SPLINE))
            .field("heatParams", class(HEAT))
            .field("quantumBoostParams", class(BOOST)),
        Layout::WithoutSpline => drive.field("heatParams", class(EMPTY_HEAT)),
        Layout::WithoutStandard => drive.field("splineJumpParams", class(SPLINE)),
        Layout::InvalidSpline => drive.field("splineJumpParams", TestType::UInt32),
    }
}

fn profile_schema(name: &str) -> TestStruct {
    number_schema(name, &PROFILE_FIELDS)
        .field("idleState", class(STATE))
        .field("travellingState", class(STATE))
        .array("effectTags", class(TAG_LINK))
        .field("fixtureUnknownFlag", TestType::Bool)
}

fn number_schema(name: &str, fields: &[(&str, &str)]) -> TestStruct {
    fields
        .iter()
        .fold(TestStruct::new(name), |schema, (name, _)| {
            schema.field(*name, TestType::Single)
        })
}

fn owner(layout: Layout) -> TestRecord {
    let attach = TestInstance::new("SAttachableComponentParams").value(
        "AttachDef",
        TestValue::Struct(
            TestInstance::new("FixtureQuantumAttach")
                .value("Type", TestValue::text("QuantumDrive")),
        ),
    );
    let mut values = vec![
        TestValue::Struct(attach),
        TestValue::Struct(drive_value(layout, 0.0)),
    ];
    if matches!(layout, Layout::Full) {
        values.push(TestValue::Struct(drive_value(layout, 2048.0)));
    }
    TestRecord::new(
        OWNER,
        "EntityClassDefinition",
        "EntityClassDefinition.fixture_quantum",
        "libs/foundry/records/entities/scitem/ships/fixture_quantum.xml",
    )
    .value("Components", TestValue::Array(values))
}

fn drive_value(layout: Layout, seed: f32) -> TestInstance {
    let mut drive = TestInstance::new(DRIVE)
        .value("quantumFuelRequirement", TestValue::Single(-0.0))
        .value("jumpRange", TestValue::Single(f32::MAX))
        .value(
            "disconnectRange",
            TestValue::Double(f64::from_bits(NONFINITE_RANGE)),
        )
        .value("fixtureRootGuid", TestValue::Guid(ROOT_GUID))
        .value("fixtureNullReference", TestValue::Null);
    if !matches!(layout, Layout::WithoutStandard) {
        drive = drive.value(
            "params",
            profile_value(STANDARD, seed + 1.0, [FIRST_TAG, SECOND_TAG]),
        );
    }
    match layout {
        Layout::Full => drive
            .value(
                "splineJumpParams",
                profile_value(SPLINE, seed + 128.0, [SECOND_TAG, FIRST_TAG]),
            )
            .value("heatParams", number_value(HEAT, &HEAT_FIELDS, seed + 256.0))
            .value(
                "quantumBoostParams",
                number_value(BOOST, &BOOST_FIELDS, seed + 512.0),
            ),
        Layout::WithoutSpline => drive.value(
            "heatParams",
            TestValue::Struct(TestInstance::new(EMPTY_HEAT)),
        ),
        Layout::WithoutStandard => drive.value(
            "splineJumpParams",
            profile_value(SPLINE, seed + 128.0, [SECOND_TAG, FIRST_TAG]),
        ),
        Layout::InvalidSpline => drive.value("splineJumpParams", TestValue::UInt32(42)),
    }
}

fn profile_value(name: &str, seed: f32, tags: [u128; 2]) -> TestValue {
    let state = |label| {
        TestValue::Struct(
            TestInstance::new(STATE).value("value", TestValue::text(format!("{name}-{label}"))),
        )
    };
    let tags = tags
        .into_iter()
        .map(|guid| {
            TestValue::Struct(
                TestInstance::new(TAG_LINK).value("tagRecord", TestValue::Reference(guid)),
            )
        })
        .collect();
    TestValue::Struct(
        number_instance(name, &PROFILE_FIELDS, seed)
            .value("idleState", state("idle"))
            .value("travellingState", state("travelling"))
            .value("effectTags", TestValue::Array(tags))
            .value("fixtureUnknownFlag", TestValue::Bool(false)),
    )
}

fn number_value(name: &str, fields: &[(&str, &str)], seed: f32) -> TestValue {
    TestValue::Struct(number_instance(name, fields, seed))
}

fn number_instance(name: &str, fields: &[(&str, &str)], mut seed: f32) -> TestInstance {
    let mut instance = TestInstance::new(name);
    for &(field, _) in fields {
        instance = instance.value(field, TestValue::Single(seed));
        seed += 1.0;
    }
    instance
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
