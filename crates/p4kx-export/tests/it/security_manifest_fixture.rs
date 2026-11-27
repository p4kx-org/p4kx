// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const MANIFEST: u128 = 0x1074_0001;
pub(super) const OWNER: u128 = 0x1074_0002;
pub(super) const LEGACY: u128 = 0x1074_0003;
pub(super) const TOKEN: u128 = 0x1074_0004;
pub(super) const TAG: u128 = 0x1074_0005;
pub(super) const MISSING: u128 = 0x1074_0006;
pub(super) const DENIED: &str = "FixtureDenied";
pub(super) const GRANTED: &str = "FixtureGranted";

const SOURCE: &str = "SecurityNetworkManifest";
const PROTOCOL: &str = "SecurityNetworkProtocol";
const PERMISSIONS: &str = "SecurityNetworkPermissions";
const PERMISSION: &str = "ESecurityNetworkPermissionValue";
const CLEARANCE: &str = "SecurityClearanceConditions";
const OVERRIDE: &str = "SecurityNetworkProtocolOverride";
const HOSTILITY: &str = "SHostilityRules";
const VARIABLE: &str = "SecurityNetworkVariable";
const VALUE: &str = "SecurityNetworkVariableValue_Base";
const BOOL: &str = "SecurityNetworkVariableValue_Bool";
const INT: &str = "SecurityNetworkVariableValue_Int";
const EFFECTS: &str = "SecurityNetworkVariableEffects";
const EFFECT: &str = "SecurityNetworkVariableEffect_Base";
const SET_STATE: &str = "SecurityNetworkVariableEffect_SetInteractionState";
const SET_RULES: &str = "SecurityNetworkVariableEffect_SetHostilityRules";
const FUTURE: &str = "FixtureFutureSecurityEffect";
const TOKEN_TYPE: &str = "SecurityClearanceToken";

pub(super) fn database() -> DataForgeBuilder {
    let database = DataForgeBuilder::new()
        .version(8)
        .enumeration(PERMISSION, [DENIED, GRANTED])
        .structure(manifest_type())
        .structure(protocol_type())
        .structure(
            TestStruct::new(PERMISSIONS)
                .field("access", TestType::Enum(PERMISSION.to_owned()))
                .field("trespass", TestType::Enum(PERMISSION.to_owned())),
        )
        .structure(TestStruct::new(CLEARANCE).array("tokens", link(TOKEN_TYPE)))
        .structure(
            TestStruct::new(OVERRIDE)
                .field("roomIdentifiers", class("TagList"))
                .field("protocol", class(PROTOCOL)),
        )
        .structure(TestStruct::new("TagList").array("tags", link("Tag")))
        .structure(hostility_type());
    variable_types(database)
        .structure(TestStruct::new("Faction"))
        .structure(TestStruct::new("Faction_LEGACY"))
        .structure(TestStruct::new("Tag"))
        .structure(TestStruct::new(TOKEN_TYPE))
        .structure(TestStruct::new("SInteractionState"))
        .record(manifest())
        .record(record(OWNER, "Faction", "outside/owner.xml"))
        .record(record(LEGACY, "Faction_LEGACY", "outside/legacy.xml"))
        .record(record(TAG, "Tag", "outside/tag.xml"))
        .record(record(TOKEN, TOKEN_TYPE, "outside/token.xml"))
        .record(TestRecord::new(
            0x1074_0007,
            "SInteractionState",
            "FixtureState",
            "outside/state.xml",
        ))
}

fn manifest_type() -> TestStruct {
    TestStruct::new(SOURCE)
        .field("defaultProtocol", class(PROTOCOL))
        .array("roomProtocols", class(OVERRIDE))
        .array("variables", class(VARIABLE))
        .array("hostilityRules", class(HOSTILITY))
        .array("variableEffects", class(EFFECTS))
        .field("isNeutralTerritory", TestType::Bool)
        .field("teleportPlayerOut", TestType::Bool)
        .field("fixtureExtra", TestType::Single)
}

fn protocol_type() -> TestStruct {
    TestStruct::new(PROTOCOL)
        .field("permissionsWithClearance", class(PERMISSIONS))
        .field("permissionsWithoutClearance", class(PERMISSIONS))
        .field("acceptedClearance", class(CLEARANCE))
        .field("ownerFaction", link("Faction"))
}

fn hostility_type() -> TestStruct {
    TestStruct::new(HOSTILITY)
        .field("tag", link("Tag"))
        .field("useReputationHostility", TestType::Bool)
        .field("usePersonalHostility", TestType::Bool)
        .field("useFactionHostility", TestType::Bool)
        .field("policesLawfulTrespass", TestType::Bool)
        .field("hostileToCriminals", TestType::Bool)
        .field("hostileToStolenVehicles", TestType::Bool)
        .field("factionOverride", link("Faction_LEGACY"))
        .field("factionToOverride", link("Faction"))
}

fn variable_types(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(
            TestStruct::new(VARIABLE)
                .field("tag", link("Tag"))
                .field("value", TestType::StrongPointer(VALUE.to_owned())),
        )
        .structure(TestStruct::new(VALUE))
        .structure(
            TestStruct::new(BOOL)
                .parent(VALUE)
                .field("value", TestType::Bool),
        )
        .structure(
            TestStruct::new(INT)
                .parent(VALUE)
                .field("value", TestType::Int32),
        )
        .structure(
            TestStruct::new(EFFECTS)
                .field("variableValue", class(VARIABLE))
                .array("effects", TestType::StrongPointer(EFFECT.to_owned())),
        )
        .structure(TestStruct::new(EFFECT))
        .structure(TestStruct::new(SET_STATE).parent(EFFECT).field(
            "state",
            TestType::WeakPointer("SInteractionState".to_owned()),
        ))
        .structure(
            TestStruct::new(SET_RULES)
                .parent(EFFECT)
                .field("rules", class(HOSTILITY)),
        )
        .structure(
            TestStruct::new(FUTURE)
                .parent(EFFECT)
                .field("payload", TestType::Int32),
        )
}

fn manifest() -> TestRecord {
    let tags = array([TestValue::Reference(TAG), TestValue::Null]);
    let room = TestInstance::new(OVERRIDE)
        .value(
            "roomIdentifiers",
            TestValue::Struct(TestInstance::new("TagList").value("tags", tags)),
        )
        .value("protocol", protocol());
    record(MANIFEST, SOURCE, "selected/network.xml")
        .value("defaultProtocol", protocol())
        .value("roomProtocols", array([TestValue::Struct(room)]))
        .value(
            "variables",
            array([
                variable(BOOL, TestValue::Bool(true)),
                variable(INT, TestValue::Int32(-7)),
                TestValue::Struct(TestInstance::new(VARIABLE)),
            ]),
        )
        .value("hostilityRules", array([hostility()]))
        .value("variableEffects", array([effects()]))
        .value("isNeutralTerritory", TestValue::Bool(true))
        .value("teleportPlayerOut", TestValue::Bool(false))
        .value("fixtureExtra", TestValue::Single(-0.0))
}

fn protocol() -> TestValue {
    let tokens = array([
        TestValue::Reference(TOKEN),
        TestValue::Null,
        TestValue::Reference(MISSING),
    ]);
    let clearance = TestInstance::new(CLEARANCE).value("tokens", tokens);
    TestValue::Struct(
        TestInstance::new(PROTOCOL)
            .value("permissionsWithClearance", permissions(GRANTED, DENIED))
            .value("permissionsWithoutClearance", permissions(DENIED, GRANTED))
            .value("ownerFaction", TestValue::Reference(OWNER))
            .value("acceptedClearance", TestValue::Struct(clearance)),
    )
}

fn permissions(access: &str, trespass: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new(PERMISSIONS)
            .value("access", TestValue::text(access))
            .value("trespass", TestValue::text(trespass)),
    )
}

fn hostility() -> TestValue {
    TestValue::Struct(
        TestInstance::new(HOSTILITY)
            .value("tag", TestValue::Reference(TAG))
            .value("useReputationHostility", TestValue::Bool(true))
            .value("useFactionHostility", TestValue::Bool(true))
            .value("hostileToCriminals", TestValue::Bool(true))
            .value("factionOverride", TestValue::Reference(LEGACY))
            .value("factionToOverride", TestValue::Reference(OWNER)),
    )
}

fn effects() -> TestValue {
    let state = TestInstance::new(SET_STATE).value("state", TestValue::pointer("FixtureState"));
    let rules = TestInstance::new(SET_RULES).value("rules", hostility());
    let future = TestInstance::new(FUTURE).value("payload", TestValue::Int32(42));
    TestValue::Struct(
        TestInstance::new(EFFECTS)
            .value("variableValue", variable(INT, TestValue::Int32(-7)))
            .value(
                "effects",
                array([
                    TestValue::Struct(state),
                    TestValue::Struct(rules),
                    TestValue::Struct(future),
                    TestValue::Null,
                ]),
            ),
    )
}

fn variable(concrete: &str, value: TestValue) -> TestValue {
    let concrete = TestInstance::new(concrete).value("value", value);
    TestValue::Struct(
        TestInstance::new(VARIABLE)
            .value("tag", TestValue::Reference(TAG))
            .value("value", TestValue::Struct(concrete)),
    )
}

fn record(guid: u128, source_type: &str, path: &str) -> TestRecord {
    TestRecord::new(guid, source_type, source_type, path)
}

fn class(target: &str) -> TestType {
    TestType::Class(target.to_owned())
}

fn link(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

fn array<const N: usize>(values: [TestValue; N]) -> TestValue {
    TestValue::Array(Vec::from(values))
}
