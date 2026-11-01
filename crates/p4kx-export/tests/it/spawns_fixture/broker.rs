// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The mission broker entry Records of the fixture: a full one that the contract names, one that
//! lies outside the broker folder, a development leftover and one that holds an unknown struct.

use p4kx_testkit::{TestInstance, TestRecord, TestValue};

use super::template::{objective, resource_order};
use super::template_parts as parts;
use super::{
    BROKER, BROKER_BAD, BROKER_ELSEWHERE, BROKER_LEFTOVER, BROKER_OBJECTIVE, MISSION_TYPE, OWNER,
    REPUTATION_REWARD, RESOURCE, SCOPE, TAG_HIJACKED, TAG_KILL, array, location, reference, ships,
    tag_list, term, text,
};

const BROKER_FOLDER: &str = "libs/foundry/records/missionbroker/";

fn flag(name: &str, value: bool) -> (String, TestValue) {
    (name.to_owned(), TestValue::Bool(value))
}

/// An empty entry; the deadline is a class every Record of the struct states.
fn entry(guid: u128, name: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "MissionBrokerEntry",
        format!("MissionBrokerEntry.{name}"),
        path,
    )
    .value("missionDeadline", deadline())
}

/// The entry the fixture's contract names, with a value in every field the catalog reads.
pub(super) fn broker() -> TestRecord {
    let flags = [
        flag("notForRelease", false),
        flag("canBeShared", true),
        flag("hasCompleteButton", false),
        flag("onlyOwnerCanComplete", true),
        flag("initiallyActive", true),
        flag("showAsOffer", true),
        flag("onceOnly", false),
        flag("lawfulMission", true),
        flag("canReacceptAfterFailing", true),
        flag("hasPersonalCooldown", true),
        flag("instanceHasLifeTime", true),
        flag("refundBuyInOnWithdraw", true),
        flag("failIfBecameCriminal", true),
        flag("preShowObjectives", true),
        flag("missionModulePerPlayer", false),
    ];
    let record = flags.into_iter().fold(
        entry(
            BROKER,
            "Haul_A",
            &format!("{BROKER_FOLDER}covalex/haul_a.xml"),
        ),
        |record, (name, value)| record.value(&name, value),
    );
    with_parts(facts(record))
}

/// The texts, links, numbers and tags of the full entry.
fn facts(record: TestRecord) -> TestRecord {
    record
        .value("playerFacingDebugName", text("Haul A"))
        .value("title", text("@place_name"))
        .value("titleHUD", text("@LOC_UNINITIALIZED"))
        .value("description", text("@place_name"))
        .value("missionGiver", text("@place_name"))
        .value("type", reference(MISSION_TYPE))
        .value("locationMissionAvailable", reference(OWNER))
        .value("owner", reference(OWNER))
        .value("missionGiverFragmentTags", text("Covalex"))
        .value("missionModule", text("Libs/Subsumption/Missions/Haul.xml"))
        .value("missionDifficulty", TestValue::Int32(3))
        .value("missionBuyInAmount", TestValue::Int32(500))
        .value("personalCooldownTime", TestValue::Single(60.0))
        .value("respawnTime", TestValue::Single(10.0))
        .value("maxInstances", TestValue::Int32(4))
        .value("instanceLifeTime", TestValue::Single(120.0))
        .value("invitationMission", reference(BROKER_ELSEWHERE))
        .value(
            "associatedMissions",
            array(vec![reference(BROKER_ELSEWHERE)]),
        )
        .value("requiredMissions", array(vec![reference(BROKER_ELSEWHERE)]))
        .value("completionTags", tag_list(&[TAG_KILL]))
        .value("missionTags", array(vec![reference(TAG_HIJACKED)]))
        .value(
            "requiredCompletedMissionTags",
            array(vec![TestValue::Struct(term(&[TAG_KILL], &[TAG_HIJACKED]))]),
        )
        .value(
            "journalEntriesToAddOnComplete",
            array(vec![reference(RESOURCE)]),
        )
}

/// The nested structures, the objectives, the modifiers and the flow of the full entry.
fn with_parts(record: TestRecord) -> TestRecord {
    record
        .value(
            "reputationPrerequisites",
            object_of(
                "ReputationPrerequisites",
                "wantedLevel",
                TestInstance::new("WantedLevel")
                    .value("minValue", TestValue::Single(0.0))
                    .value("maxValue", TestValue::Single(2.0)),
            ),
        )
        .value("availableDateSchedule", array(vec![schedule()]))
        .value("missionReward", reward())
        .value("reputationRequirements", requirements())
        .value("missionResultReputationRewards", reputation_results())
        .value(
            "properties",
            array(vec![
                ships("Wave_BP", "", Some(9)),
                parts::named(location("Place_BP", "Place", None), "bprop_place"),
            ]),
        )
        .value("objectiveTokens", objectives())
        .value("partialRewardPayout", parts::payout())
        .value("modifiers", modifiers())
        .value("missionFlow", flow())
}

fn object_of(name: &str, field: &str, inner: TestInstance) -> TestValue {
    TestValue::Struct(TestInstance::new(name).value(field, TestValue::Struct(inner)))
}

fn date(year: i32, month: &str, day: i32) -> TestInstance {
    TestInstance::new("DateStamp")
        .value("year", TestValue::Int32(year))
        .value("month", text(month))
        .value("day", TestValue::Int32(day))
}

fn stamp(date: TestInstance, hour: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("DateTimeStamp")
            .value("date", TestValue::Struct(date))
            .value(
                "time",
                TestValue::Struct(
                    TestInstance::new("TimeStamp")
                        .value("hour", TestValue::Int32(hour))
                        .value("minute", TestValue::Int32(30))
                        .value("second", TestValue::Int32(0)),
                ),
            ),
    )
}

/// A window from 1 December 08:30 to 8 January 17:30 that repeats yearly, and a month the
/// catalog does not know as its repeat end.
fn schedule() -> TestValue {
    TestValue::Struct(
        TestInstance::new("DateSchedule")
            .value("start", stamp(date(2950, "December", 1), 8))
            .value("end", stamp(date(2951, "January", 8), 17))
            .value("repeatEnd", TestValue::Struct(date(2999, "Smarch", 1)))
            .value("repeatType", text("RepeatYearly")),
    )
}

fn reward() -> TestValue {
    TestValue::Struct(
        TestInstance::new("BrokerReward")
            .value("currencyType", text("UEC"))
            .value("reward", TestValue::Int32(12_000))
            .value("max", TestValue::Int32(15_000))
            .value("plusBonuses", TestValue::Bool(true))
            .value("reputationBonus", reference(SCOPE)),
    )
}

fn deadline() -> TestValue {
    TestValue::Struct(
        TestInstance::new("BrokerDeadline")
            .value("missionAutoEnd", TestValue::Bool(true))
            .value("missionCompletionTime", TestValue::Single(45.5))
            .value("missionEndReason", text("@place_name"))
            .value("missionResultAfterTimerEnd", text("Failed"))
            .value("remainingTimeToShowTimer", TestValue::Single(5.0)),
    )
}

/// A requirement, `and`, parentheses around a second requirement.
fn requirements() -> TestValue {
    let requirement = |comparison: &str, standing: u128| {
        TestValue::Struct(
            TestInstance::new("SReputationMissionGiverRequirementParams")
                .value("comparison", text(comparison))
                .value("factionReputation", reference(OWNER))
                .value("reputationScope", reference(SCOPE))
                .value("standing", reference(standing)),
        )
    };
    let marker = |name: &str| TestValue::Struct(TestInstance::new(name));
    TestValue::Struct(
        TestInstance::new("SReputationMissionRequirementsParams").value(
            "expression",
            array(vec![
                requirement("GreaterThanOrEqualTo", MISSION_TYPE),
                marker("SReputationMissionRequirementExpression_And"),
                marker("SReputationMissionRequirementExpression_LeftParenthesis"),
                requirement("Sideways", MISSION_TYPE),
                marker("SReputationMissionRequirementExpression_RightParenthesis"),
            ]),
        ),
    )
}

/// Two results: the first grants an amount of a reward Record, the second nothing.
fn reputation_results() -> TestValue {
    array(vec![
        TestValue::Struct(TestInstance::new("ReputationResultSet").value(
            "reputationAmounts",
            array(vec![TestValue::Struct(
                TestInstance::new("ReputationAmountEntry")
                    .value("factionReputation", reference(OWNER))
                    .value("reputationScope", reference(SCOPE))
                    .value("reward", reference(REPUTATION_REWARD)),
            )]),
        )),
        TestValue::Struct(TestInstance::new("ReputationResultSet")),
    ])
}

fn objectives() -> TestValue {
    array(vec![objective(
        (BROKER_OBJECTIVE, "btok_a"),
        ("Haul", true),
        parts::hauling_handler(resource_order(RESOURCE, 8)),
        (Vec::new(), Vec::new()),
    )])
}

fn modifiers() -> TestValue {
    let header = |name: &str| TestInstance::new(name).value("enabled", TestValue::Bool(true));
    array(vec![
        TestValue::Struct(
            header("MissionModifier_FactionHostility")
                .value("modifierName", text("Rivals"))
                .value("faction", reference(OWNER))
                .value("myReaction", text("Hostile"))
                .value("theirReaction", text("Gloating"))
                .value("ignoreCriminalHostility", TestValue::Bool(true)),
        ),
        TestValue::Struct(
            header("MissionModifier_HostileMission")
                .value("missionBrokerEntry", reference(BROKER_ELSEWHERE))
                .value("legalToAttack", TestValue::Bool(true)),
        ),
        TestValue::Struct(
            header("MissionModifier_IgnoreMissionPlayerCriminality")
                .value("missionBrokerEntry", reference(BROKER_ELSEWHERE)),
        ),
        TestValue::Struct(
            header("MissionModifier_LawLicense")
                .value("licenseType", text("IllegalGoodsLicense_ClassB")),
        ),
    ])
}

fn flow() -> TestValue {
    parts::object(TestInstance::new("MissionFlow").value(
        "triggers",
        array(vec![parts::trigger(
            "Done",
            parts::token_state("Completed", Some("btok_a")),
            TestInstance::new("MissionFlowAction_StateChange").value("toState", text("Completed")),
        )]),
    ))
}

/// An entry outside the broker folder that the full entry links to; every field is absent.
pub(super) fn elsewhere() -> TestRecord {
    entry(
        BROKER_ELSEWHERE,
        "Elsewhere",
        "libs/foundry/records/mission/other/elsewhere.xml",
    )
}

/// An entry whose record path marks it as a development leftover.
pub(super) fn leftover() -> TestRecord {
    entry(
        BROKER_LEFTOVER,
        "Dev_Temp",
        &format!("{BROKER_FOLDER}dev_temp.xml"),
    )
}

/// An entry whose reputation requirements are a struct the catalog does not know.
pub(super) fn bad() -> TestRecord {
    entry(BROKER_BAD, "Bad", &format!("{BROKER_FOLDER}bad.xml")).value(
        "reputationRequirements",
        TestValue::Struct(TestInstance::new("SReputationMissionRequirementsUnknown")),
    )
}

/// The reward Record the first reputation result names.
pub(super) fn reward_record() -> TestRecord {
    TestRecord::new(
        REPUTATION_REWARD,
        "ReputationReward",
        "ReputationReward.Plus_Two",
        "libs/foundry/records/reputation/rewards/missionrewards_reputation/plus_two.xml",
    )
    .value("reputationAmount", TestValue::Int32(2))
}
