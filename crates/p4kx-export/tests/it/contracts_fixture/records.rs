// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestValue};

use super::gaps;
use super::lookups::{amount, bounty_params, empty_payout, legacy, payout};
use super::prerequisites::{locality, location, standing_prerequisite};
use super::{
    AFFINITY, BROKER, CONTRACT_A, CONTRACT_B, CONTRACT_C, CONTRACT_D, CONTRACT_DEV, CONTRACT_E,
    CONTRACT_F, CONTRACT_G, CONTRACT_H, CONTRACT_I, ESCORT, FACTION, FACTION_SCOPE, GENERATOR,
    GENERATOR_COPY, GENERATOR_DEV, LOC_A, LOC_B, LOSS, MISSING_TEMPLATE, OBJ_PLANET, ORGANIZATION,
    PLAIN_TEMPLATE, RANK_FIFTY, RANK_TWO_HUNDRED, SOLO_ORGANIZATION, STRAY_TEMPLATE, SUB_ONE,
    TEMPLATE,
};

pub(super) const fn reference(guid: u128) -> TestValue {
    TestValue::Reference(guid)
}

fn param(name: &str, key: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ContractStringParam")
            .value("param", TestValue::text(name))
            .value("value", TestValue::text(key)),
    )
}

pub(super) fn property(token: &str, value: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new("MissionProperty")
            .value(
                "missionVariableName",
                TestValue::text(format!("Var{token}")),
            )
            .value("extendedTextToken", TestValue::text(token))
            .value("value", value),
    )
}

pub(super) fn string_hash(key: &str) -> TestValue {
    TestValue::Struct(TestInstance::new("MissionPropertyValue_StringHash").value(
        "options",
        TestValue::Array(vec![TestValue::Struct(
                TestInstance::new("MissionPropertyValueOption_StringHash")
                    .value("textId", TestValue::text(key))
                    .value("weighting", TestValue::Single(1.0)),
            )]),
    ))
}

pub(super) fn organization(guid: u128) -> TestValue {
    TestValue::Struct(
        TestInstance::new("MissionPropertyValue_Organization").value(
            "matchConditions",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new("DataSetMatchCondition_SpecificOrganizationsDef")
                    .value("organizations", TestValue::Array(vec![reference(guid)])),
            )]),
        ),
    )
}

pub(super) fn plain(kind: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(kind))
}

fn overrides(
    title: Option<&str>,
    mission_type: Option<u128>,
    properties: Vec<TestValue>,
) -> TestValue {
    let mut params = TestInstance::new("ContractParamOverrides");
    if let Some(guid) = mission_type {
        params = params.value("missionTypeOverride", reference(guid));
    }
    TestValue::Struct(
        params
            .value(
                "stringParamOverrides",
                TestValue::Array(title.map(|key| param("Title", key)).into_iter().collect()),
            )
            .value("propertyOverrides", TestValue::Array(properties)),
    )
}

pub(super) fn results(results: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ContractResultBlock")
            .value("contractResults", TestValue::Array(results)),
    )
}

fn contract(id: u128, name: &str, template: u128, params: TestValue) -> TestInstance {
    TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(id))
        .value("debugName", TestValue::text(name))
        .value("template", reference(template))
        .value("paramOverrides", params)
}

fn sub(id: u128, params: TestValue, prerequisites: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SubContract")
            .value("id", TestValue::Guid(id))
            .value("debugName", TestValue::text("Sub_Contract"))
            .value("additionalPrerequisites", TestValue::Array(prerequisites))
            .value("paramOverrides", params),
    )
}

fn handler(
    name: &str,
    params: TestValue,
    contracts: Vec<TestValue>,
    intro: Vec<TestValue>,
    faction: u128,
    prerequisites: Vec<TestValue>,
) -> TestValue {
    let prerequisites = TestValue::Array(prerequisites);
    let (handler_type, availability) = match name {
        "Delivery_Career" => (
            "ContractGeneratorHandler_Career",
            TestInstance::new("ContractAvailability")
                .value("prerequisites", prerequisites)
                .value("hasPersonalCooldown", TestValue::Bool(true))
                .value("personalCooldownTime", TestValue::Single(2880.0))
                .value("personalCooldownTimeVariation", TestValue::Single(60.0))
                .value("abandonedCooldownTime", TestValue::Single(30.0))
                .value("abandonedCooldownTimeVariation", TestValue::Single(0.5)),
        ),
        "Copy_Career" => (
            "ContractGeneratorHandler_PVPBountyDef",
            TestInstance::new("ContractAvailabilityPartial")
                .value("prerequisites", prerequisites)
                .value("hasPersonalCooldown", TestValue::Bool(false))
                .value("personalCooldownTime", TestValue::Int32(45)),
        ),
        _ => (
            "ContractGeneratorHandler_List",
            TestInstance::new("ContractAvailabilityWithoutCooldown")
                .value("prerequisites", prerequisites),
        ),
    };
    TestValue::Struct(
        TestInstance::new(handler_type)
            .value("debugName", TestValue::text(name))
            .value("factionReputation", reference(faction))
            .value("defaultAvailability", TestValue::Struct(availability))
            .value("contractParams", params)
            .value("contracts", TestValue::Array(contracts))
            .value("introContracts", TestValue::Array(intro)),
    )
}

fn delivery_contract() -> TestInstance {
    contract(
        CONTRACT_A,
        "Deliver_A",
        TEMPLATE,
        overrides(
            Some("@title_a"),
            None,
            vec![property("Cargo", string_hash("@cargo_scrap"))],
        ),
    )
    .value(
        "subContracts",
        TestValue::Array(vec![
            sub(
                SUB_ONE,
                overrides(Some("@title_sub"), Some(ESCORT), Vec::new()),
                vec![
                    location(OBJ_PLANET),
                    standing_prerequisite(true, Some((RANK_TWO_HUNDRED, RANK_FIFTY))),
                ],
            ),
            gaps::sub_two(),
        ]),
    )
    .value(
        "additionalPrerequisites",
        TestValue::Array(vec![
            locality(LOC_A),
            standing_prerequisite(false, Some((RANK_FIFTY, RANK_TWO_HUNDRED))),
        ]),
    )
    .value("minStanding", reference(RANK_FIFTY))
    .value("maxStanding", reference(RANK_TWO_HUNDRED))
    .value(
        "contractResults",
        results(vec![
            legacy(
                [true, false, false, false, false],
                vec![
                    amount(FACTION_SCOPE, RANK_TWO_HUNDRED),
                    amount(AFFINITY, RANK_FIFTY),
                ],
            ),
            legacy(
                [false, false, true, false, false],
                vec![amount(FACTION_SCOPE, LOSS)],
            ),
            payout([true, false, false, false, false], 40_000),
            payout([false, true, false, false, false], 1_000),
            empty_payout([true, false, false, false, false]),
            TestValue::Struct(TestInstance::new("ContractResult_CompletionTags")),
            gaps::blueprint(true, gaps::POOL_CONTRACT, 0.25),
            gaps::blueprint(false, gaps::POOL_UNREACHED, 1.0),
        ]),
    )
}

fn contractor_params(key: &str) -> TestValue {
    TestValue::Struct(TestInstance::new("ContractParamOverrides").value(
        "stringParamOverrides",
        TestValue::Array(vec![param("Contractor", key)]),
    ))
}

fn bounty_handler() -> TestValue {
    let bounty = contract(
        CONTRACT_C,
        "Bounty_C",
        PLAIN_TEMPLATE,
        gaps::modifier_overrides(),
    );
    let leftover = contract(
        CONTRACT_DEV,
        "Test_Contract",
        TEMPLATE,
        overrides(Some("@title_a"), None, Vec::new()),
    );
    let without_id = TestInstance::new("CareerContract")
        .value("debugName", TestValue::text("Deliver_NoId"))
        .value("template", reference(TEMPLATE));
    handler(
        "Bounty_Career",
        bounty_params(),
        vec![TestValue::Struct(bounty)],
        vec![TestValue::Struct(leftover), TestValue::Struct(without_id)],
        0,
        Vec::new(),
    )
}

fn delivery_handler() -> TestValue {
    let delivery = delivery_contract();
    let bare = contract(
        CONTRACT_B,
        "Deliver_B",
        TEMPLATE,
        overrides(None, None, Vec::new()),
    )
    .value(
        "additionalPrerequisites",
        TestValue::Array(vec![locality(LOC_B)]),
    )
    .value(
        "subContracts",
        TestValue::Array(vec![
            TestValue::Struct(
                TestInstance::new("SubContract")
                    .value("id", TestValue::Guid(SUB_ONE + 1))
                    .value("debugName", TestValue::text("Test_Sub")),
            ),
            TestValue::Struct(TestInstance::new("SubContract")),
        ]),
    );
    let nested = contract(
        CONTRACT_D,
        "Deliver_D",
        TEMPLATE,
        overrides(
            Some("@title_nested"),
            None,
            vec![property("Wrap", string_hash("@wrap_text"))],
        ),
    )
    .value("additionalPrerequisites", gaps::odd_locations());
    let brokered = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(CONTRACT_E))
        .value("debugName", TestValue::text("Haul_E"))
        .value("missionBrokerEntry", reference(BROKER));
    handler(
        "Delivery_Career",
        contractor_params("@contractor_acme"),
        vec![
            TestValue::Struct(delivery),
            TestValue::Struct(bare),
            TestValue::Struct(nested),
            TestValue::Struct(brokered),
        ],
        Vec::new(),
        FACTION,
        vec![standing_prerequisite(false, None)],
    )
}

fn copy_handler() -> TestValue {
    let again = contract(
        CONTRACT_A,
        "Deliver_A_Again",
        TEMPLATE,
        overrides(Some("@title_handler"), None, Vec::new()),
    )
    .value(
        "subContracts",
        TestValue::Array(vec![sub(
            SUB_ONE,
            overrides(None, None, Vec::new()),
            Vec::new(),
        )]),
    );
    let dangling = contract(
        CONTRACT_G,
        "Deliver_G",
        MISSING_TEMPLATE,
        overrides(Some("@title_handler"), None, Vec::new()),
    )
    .value("additionalPrerequisites", gaps::exclusion());
    let stray = contract(
        CONTRACT_H,
        "Deliver_H",
        STRAY_TEMPLATE,
        overrides(None, None, Vec::new()),
    )
    .value("additionalPrerequisites", gaps::requirement())
    .value("contractResults", gaps::rival_only());
    let organisations = contract(
        CONTRACT_I,
        "Deliver_I",
        PLAIN_TEMPLATE,
        overrides(
            Some("@title_novariant"),
            None,
            vec![
                property("Several", organization(ORGANIZATION)),
                property("Solo", organization(SOLO_ORGANIZATION)),
            ],
        ),
    )
    .value("contractResults", gaps::rival_then_gain());
    handler(
        "Copy_Career",
        overrides(None, None, Vec::new()),
        vec![
            TestValue::Struct(again),
            TestValue::Struct(dangling),
            TestValue::Struct(stray),
            TestValue::Struct(organisations),
        ],
        Vec::new(),
        0,
        Vec::new(),
    )
}

fn event_handler() -> TestValue {
    let event = contract(
        CONTRACT_F,
        "Event_F",
        TEMPLATE,
        overrides(Some("@title_a"), None, Vec::new()),
    );
    handler(
        "Event_Career",
        overrides(None, None, Vec::new()),
        vec![TestValue::Struct(event)],
        Vec::new(),
        0,
        Vec::new(),
    )
}

fn generator_record(guid: u128, name: &str, handlers: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        guid,
        "ContractGenerator",
        format!("ContractGenerator.{name}"),
        format!("libs/foundry/records/contracts/contractgenerator/{name}.xml"),
    )
    .value("generators", TestValue::Array(handlers))
}

pub(super) fn generator(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .record(generator_record(
            GENERATOR,
            "delivery",
            vec![delivery_handler(), bounty_handler()],
        ))
        .record(generator_record(
            GENERATOR_COPY,
            "delivery_copy",
            vec![copy_handler()],
        ))
        .record(generator_record(
            GENERATOR_DEV,
            "dev_events",
            vec![event_handler()],
        ))
}
