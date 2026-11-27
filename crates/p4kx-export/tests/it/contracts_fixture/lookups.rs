// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestValue};

use super::{
    AFFINITY, BOUNTY, BROKER, DELIVERY, FACTION, FACTION_SCOPE, LOC_A, LOC_B, LOSS, OBJ_PLANET,
    OBJ_STAR, OBJ_STATION, OBJ_SYSTEM, RANK_FIFTY, RANK_TWO_HUNDRED, RIVAL, STRAY_FACTION,
    TYPE_PLANET, TYPE_STAR, TYPE_STATION, TYPE_SYSTEM,
};

pub(super) fn flags(set: [bool; 5]) -> TestValue {
    TestValue::Array(
        set.into_iter()
            .map(|value| {
                TestValue::Struct(TestInstance::new("Bool").value("value", TestValue::Bool(value)))
            })
            .collect(),
    )
}

/// The overrides of the bounty handler: a title, a description and the bounty mission type.
pub(super) fn bounty_params() -> TestValue {
    let string = |name: &str, key: &str| {
        TestValue::Struct(
            TestInstance::new("ContractStringParam")
                .value("param", TestValue::text(name))
                .value("value", TestValue::text(key)),
        )
    };
    TestValue::Struct(
        TestInstance::new("ContractParamOverrides")
            .value("missionTypeOverride", TestValue::Reference(BOUNTY))
            .value(
                "stringParamOverrides",
                TestValue::Array(vec![
                    string("Title", "@title_handler"),
                    string("Description", "@desc_handler"),
                ]),
            )
            .value("propertyOverrides", TestValue::Array(Vec::new())),
    )
}

pub(super) fn amount(scope: u128, reward: u128) -> TestValue {
    amount_of(FACTION, scope, reward)
}

pub(super) fn amount_of(faction: u128, scope: u128, reward: u128) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ReputationAmount")
            .value("factionReputation", TestValue::Reference(faction))
            .value("reputationScope", TestValue::Reference(scope))
            .value("reward", TestValue::Reference(reward)),
    )
}

pub(super) fn legacy(set: [bool; 5], amounts: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ContractResult_LegacyReputation")
            .value("missionResults", flags(set))
            .value("contractResultReputationAmounts", TestValue::Array(amounts)),
    )
}

pub(super) fn payout(set: [bool; 5], amount: i32) -> TestValue {
    let reward = TestInstance::new("MissionRewardWithCurrency")
        .value("reward", TestValue::Int32(amount))
        .value("max", TestValue::Int32(0))
        .value("plusBonuses", TestValue::Bool(false))
        .value("currencyType", TestValue::text("UEC"));
    TestValue::Struct(
        TestInstance::new("ContractResult_Reward")
            .value("missionResults", flags(set))
            .value("contractReward", TestValue::Struct(reward)),
    )
}

pub(super) fn empty_payout(set: [bool; 5]) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ContractResult_Reward")
            .value("missionResults", flags(set))
            .value(
                "contractReward",
                TestValue::Struct(
                    TestInstance::new("MissionReward").value("reward", TestValue::Int32(7)),
                ),
            ),
    )
}

pub(super) fn plain_record(guid: u128, kind: &str, path: &str) -> TestRecord {
    let name = path.rsplit('/').next().unwrap_or(path);
    TestRecord::new(
        guid,
        kind,
        format!("{kind}.{name}"),
        format!("libs/foundry/records/{path}.xml"),
    )
}

pub(super) fn mission_type(guid: u128, name: &str, key: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "MissionType",
        format!("MissionType.{name}"),
        format!("libs/foundry/records/missiontype/{name}.xml"),
    )
    .value("LocalisedTypeName", TestValue::text(key))
}

fn reward(guid: u128, name: &str, value: i32) -> TestRecord {
    plain_record(
        guid,
        "ReputationReward",
        &format!("reputation/rewards/missionrewards_reputation/{name}"),
    )
    .value("reputationAmount", TestValue::Int32(value))
}

fn object(guid: u128, name: &str, kind: u128) -> TestRecord {
    plain_record(guid, "StarMapObject", &format!("starmap/pu/{name}"))
        .value("name", TestValue::text(format!("@place_{name}")))
        .value("type", TestValue::Reference(kind))
}

fn kind(guid: u128, name: &str) -> TestRecord {
    plain_record(guid, "StarMapObjectType", &format!("starmap/type/{name}"))
        .value("name", TestValue::text(name))
}

fn mission_locality(guid: u128, name: &str, locations: &[u128]) -> TestRecord {
    plain_record(
        guid,
        "MissionLocality",
        &format!("missiondata/pu_missionlocality/{name}"),
    )
    .value(
        "availableLocations",
        TestValue::Array(
            locations
                .iter()
                .copied()
                .map(TestValue::Reference)
                .collect(),
        ),
    )
}

pub(super) fn places(builder: DataForgeBuilder) -> DataForgeBuilder {
    let stations: Vec<u128> = (0..7).map(|offset| OBJ_STATION + offset).collect();
    let mut records = vec![
        kind(TYPE_SYSTEM, "SolarSystem"),
        kind(TYPE_PLANET, "Planet"),
        kind(TYPE_STAR, "Star"),
        kind(TYPE_STATION, "Station"),
        object(OBJ_SYSTEM, "stanton", TYPE_SYSTEM),
        object(OBJ_PLANET, "hurston", TYPE_PLANET),
        object(OBJ_STAR, "sol", TYPE_STAR),
        mission_locality(
            LOC_A,
            "stanton_all",
            &[OBJ_STATION, OBJ_STAR, OBJ_PLANET, OBJ_SYSTEM],
        ),
        mission_locality(LOC_B, "many_stations", &stations),
    ];
    records.extend(
        stations
            .iter()
            .enumerate()
            .map(|(number, guid)| object(*guid, &format!("station{number}"), TYPE_STATION)),
    );
    records.into_iter().fold(builder, DataForgeBuilder::record)
}

pub(super) fn faction_scope() -> TestRecord {
    plain_record(
        FACTION_SCOPE,
        "ReputationScope",
        "reputation/scopes/faction",
    )
}

pub(super) fn reputation(builder: DataForgeBuilder, scope: TestRecord) -> DataForgeBuilder {
    [
        plain_record(
            FACTION,
            "FactionReputation",
            "factions/factionreputation/acme",
        )
        .value("displayName", TestValue::text("@faction_acme")),
        plain_record(STRAY_FACTION, "FactionReputation", "factions/stray/odd"),
        plain_record(
            RIVAL,
            "FactionReputation",
            "factions/factionreputation/rival",
        ),
        plain_record(BROKER, "MissionBrokerEntry", "missionbroker/haul")
            .value("title", TestValue::text("@title_broker"))
            .value("description", TestValue::text("@desc_broker"))
            .value("type", TestValue::Reference(DELIVERY))
            .value("locationMissionAvailable", TestValue::Reference(OBJ_PLANET)),
        scope,
        plain_record(AFFINITY, "ReputationScope", "reputation/scopes/affinity"),
        reward(RANK_TWO_HUNDRED, "gain_200", 200),
        reward(RANK_FIFTY, "gain_50", 50),
        reward(LOSS, "loss_100", -100),
    ]
    .into_iter()
    .fold(builder, DataForgeBuilder::record)
}
