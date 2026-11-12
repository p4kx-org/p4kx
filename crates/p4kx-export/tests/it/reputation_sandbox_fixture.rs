// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const OWNER: u128 = 0x1073_0901;
pub(super) const EMPTY_OWNER: u128 = 0x1073_0907;
pub(super) const RECORD: u128 = 0x1073_0902;
pub(super) const REWARD: u128 = 0x1073_0903;
pub(super) const SCOPE: u128 = 0x1073_0904;
pub(super) const INFRACTION: u128 = 0x1073_0905;
pub(super) const MISSING_FACTION: u128 = 0x1073_0906;

const FACTION: &str = "FactionReputation";
const SCOPE_TYPE: &str = "SReputationScopeParams";
const TRIGGER_BASE: &str = "SandboxTriggerBaseDef";
const MANUAL: &str = "SandboxTriggerManualParams";
pub(super) const FUTURE: &str = "FixtureSandboxTriggerConditions";
const CONDITION: &str = "FixtureSandboxCondition";
const RECORD_USE: &str = "SandboxTriggerRecordParams";
const RECORD_TYPE: &str = "SandboxTriggerRecord";
const INFRACTION_BASE: &str = "SandboxInfractionBaseDef";
const EVENT_TRIGGER: &str = "SandboxTrigger";
const INFRACTION_USE: &str = "SandboxInfractionDefinitionTrigger";
const INFRACTION_TYPE: &str = "InfractionDefinition";
const EVENT_ENUM: &str = "EInfractionTrigger";
pub(super) const EVENT_LABEL: &str = "FixtureFutureInfraction";
const REWARD_BASE: &str = "ReputationRewardBaseDef";
const REWARD_FACTION: &str = "ReputationRewardFaction";
const REWARD_TYPE: &str = "SReputationRewardAmount";

pub(super) fn database() -> DataForgeBuilder {
    let db = schema().fold(
        DataForgeBuilder::new()
            .version(8)
            .enumeration(EVENT_ENUM, [EVENT_LABEL]),
        DataForgeBuilder::structure,
    );
    db.record(faction())
        .record(
            record(EMPTY_OWNER, FACTION, "factions/factionreputation/empty")
                .value("sandboxTriggers", TestValue::Array(Vec::new())),
        )
        .record(
            record(RECORD, RECORD_TYPE, "reputation/triggers/fixture").value(
                "triggerManualParams",
                TestValue::Array(vec![manual(
                    TestInstance::new(INFRACTION_USE).value(
                        "sandboxInfractionDefinitionTrigger",
                        TestValue::Reference(INFRACTION),
                    ),
                    false,
                )]),
            ),
        )
        .record(
            record(REWARD, REWARD_TYPE, "reputation/rewards/fixture")
                .value("editorName", TestValue::text("Fixture reward"))
                .value("reputationAmount", TestValue::Int32(-32)),
        )
        .record(record(SCOPE, SCOPE_TYPE, "reputation/scopes/fixture"))
        .record(record(
            INFRACTION,
            INFRACTION_TYPE,
            "law/infractions/fixture",
        ))
}

fn schema() -> impl Iterator<Item = TestStruct> {
    [
        TestStruct::new(FACTION).array("sandboxTriggers", pointer(TRIGGER_BASE)),
        TestStruct::new(SCOPE_TYPE),
        TestStruct::new(TRIGGER_BASE),
        TestStruct::new(MANUAL)
            .parent(TRIGGER_BASE)
            .field("infractionTrigger", pointer(INFRACTION_BASE))
            .field("triggerOnInnocentsOnly", TestType::Bool)
            .array("outcomes", pointer(REWARD_BASE)),
        TestStruct::new(FUTURE)
            .parent(TRIGGER_BASE)
            .field("predicate", TestType::Class(CONDITION.to_owned())),
        TestStruct::new(CONDITION).field("negated", TestType::Bool),
        TestStruct::new(RECORD_USE)
            .parent(TRIGGER_BASE)
            .field("triggerRecord", reference(RECORD_TYPE)),
        TestStruct::new(RECORD_TYPE)
            .array("triggerManualParams", TestType::Class(MANUAL.to_owned())),
        TestStruct::new(INFRACTION_BASE),
        TestStruct::new(EVENT_TRIGGER)
            .parent(INFRACTION_BASE)
            .field("sandboxTrigger", TestType::Enum(EVENT_ENUM.to_owned())),
        TestStruct::new(INFRACTION_USE)
            .parent(INFRACTION_BASE)
            .field(
                "sandboxInfractionDefinitionTrigger",
                reference(INFRACTION_TYPE),
            ),
        TestStruct::new(INFRACTION_TYPE),
        TestStruct::new(REWARD_BASE)
            .field("scope", reference(SCOPE_TYPE))
            .field("reward", reference(REWARD_TYPE)),
        TestStruct::new(REWARD_FACTION)
            .parent(REWARD_BASE)
            .field("factionReputation", reference(FACTION)),
        TestStruct::new(REWARD_TYPE)
            .field("editorName", TestType::String)
            .field("reputationAmount", TestType::Int32),
    ]
    .into_iter()
}

fn faction() -> TestRecord {
    record(OWNER, FACTION, "factions/factionreputation/fixture").value(
        "sandboxTriggers",
        TestValue::Array(vec![
            manual(
                TestInstance::new(EVENT_TRIGGER)
                    .value("sandboxTrigger", TestValue::text(EVENT_LABEL)),
                true,
            ),
            TestValue::Null,
            TestValue::Struct(
                TestInstance::new(RECORD_USE).value("triggerRecord", TestValue::Reference(RECORD)),
            ),
            TestValue::Struct(TestInstance::new(FUTURE).value(
                "predicate",
                TestValue::Struct(
                    TestInstance::new(CONDITION).value("negated", TestValue::Bool(true)),
                ),
            )),
        ]),
    )
}

fn manual(infraction: TestInstance, innocents_only: bool) -> TestValue {
    TestValue::Struct(
        TestInstance::new(MANUAL)
            .value("infractionTrigger", TestValue::Struct(infraction))
            .value("triggerOnInnocentsOnly", TestValue::Bool(innocents_only))
            .value(
                "outcomes",
                TestValue::Array(vec![
                    faction_outcome(MISSING_FACTION),
                    faction_outcome(OWNER),
                ]),
            ),
    )
}

fn faction_outcome(target: u128) -> TestValue {
    TestValue::Struct(
        TestInstance::new(REWARD_FACTION)
            .value("scope", TestValue::Reference(SCOPE))
            .value("reward", TestValue::Reference(REWARD))
            .value("factionReputation", TestValue::Reference(target)),
    )
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

fn record(guid: u128, source_type: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        source_type,
        format!("{source_type}.Fixture"),
        format!("libs/foundry/records/{path}.xml"),
    )
}
