// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(crate) const DEFINITION: u128 = 0x0010_7201;
pub(crate) const SET: u128 = 0x0010_7202;
pub(crate) const PRISON: u128 = 0x0010_7203;
pub(crate) const PARENT: u128 = 0x0010_7204;
pub(crate) const MISSING: u128 = 0x0010_7205;
pub(crate) const JOURNAL: u128 = 0x0010_7206;
pub(crate) const ENTITY: u128 = 0x0010_7207;
pub(crate) const RESOURCE: u128 = 0x0010_7208;
pub(crate) const FUTURE: &str = "FixtureFutureTrigger";
pub(crate) const FIRST: &str = "FixtureFirstTrigger";

const DEFINITION_TYPE: &str = "InfractionDefinition";
const SET_TYPE: &str = "InfractionSet";
const USE_TYPE: &str = "Infraction";
const PARAMETERS: &str = "InfractionParameters";
const JURISDICTION: &str = "Jurisdiction";
const SUBSTANCE: &str = "ControlledSubstanceClass";
const IMPOUND: &str = "ImpoundingDefinition";
const CONDITION: &str = "FixtureCondition";
const NEGATION: &str = "FixtureNegation";
const TRIGGER: &str = "EInfractionTrigger";
const WRONG_TRIGGER: &str = "FixtureOtherTrigger";
const IMPOUND_TRIGGER: &str = "EImpoundingTrigger";

pub(crate) fn database(trigger_target: &str) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration(TRIGGER, [FIRST, FUTURE])
        .enumeration(WRONG_TRIGGER, [FIRST, FUTURE])
        .enumeration(IMPOUND_TRIGGER, [FUTURE])
        .structure(
            TestStruct::new(DEFINITION_TYPE)
                .field("name", TestType::Locale)
                .field("description", TestType::Locale)
                .array("triggers", TestType::Enum(trigger_target.to_owned()))
                .field("defaultParameters", class(PARAMETERS)),
        )
        .structure(parameters_declaration())
        .structure(TestStruct::new(CONDITION).array("negated", class(NEGATION)))
        .structure(
            TestStruct::new(NEGATION)
                .field("allow", TestType::Bool)
                .field("scale", TestType::Single),
        )
        .structure(
            TestStruct::new(USE_TYPE)
                .field("definition", reference(DEFINITION_TYPE))
                .field("parameterOverrides", class(PARAMETERS)),
        )
        .structure(TestStruct::new(SET_TYPE).array("infractions", class(USE_TYPE)))
        .structure(jurisdiction_declaration())
        .structure(
            TestStruct::new(SUBSTANCE)
                .array("commodities", reference("EntityClassDefinition"))
                .array("resources", reference("ResourceType"))
                .field("maxPossessionSCU", TestType::Single),
        )
        .structure(
            TestStruct::new(IMPOUND)
                .field("name", TestType::Locale)
                .field("trigger", TestType::Enum(IMPOUND_TRIGGER.to_owned()))
                .field("impoundingTimeSeconds", TestType::Single)
                .field("impoundingFineUEC", TestType::Int32)
                .field("ignoreIfAgainstPartyMember", TestType::Bool),
        )
        .structure(TestStruct::new("JournalEntry"))
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(TestStruct::new("ResourceType"))
        .record(definition())
        .record(set())
        .record(prison())
        .record(
            record(PARENT, JURISDICTION, "outside/jurisdiction/parent.xml")
                .value("parentJurisdiction", TestValue::Reference(PRISON)),
        )
        .record(record(JOURNAL, "JournalEntry", "outside/journal.xml"))
        .record(record(
            ENTITY,
            "EntityClassDefinition",
            "outside/entity.xml",
        ))
        .record(record(RESOURCE, "ResourceType", "outside/resource.xml"))
}

pub(crate) fn correct() -> DataForgeBuilder {
    database(TRIGGER)
}

pub(crate) fn wrong_trigger_declaration() -> DataForgeBuilder {
    database(WRONG_TRIGGER)
}

pub(crate) fn sparse() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(DEFINITION_TYPE)
                .field("description", TestType::Locale)
                .field("defaultParameters", class(PARAMETERS)),
        )
        .structure(TestStruct::new(PARAMETERS).array("graceWarnings", TestType::Locale))
        .structure(
            TestStruct::new(JURISDICTION)
                .field("parentJurisdiction", reference(JURISDICTION))
                .array("prohibitedGoods", reference("EntityClassDefinition")),
        )
        .structure(TestStruct::new("EntityClassDefinition"))
        .record(
            record(DEFINITION, DEFINITION_TYPE, "sparse/definition.xml")
                .value("description", unreadable_locale())
                .value(
                    "defaultParameters",
                    TestValue::Struct(TestInstance::new(PARAMETERS).value(
                        "graceWarnings",
                        TestValue::Array(vec![TestValue::text("@shared"), unreadable_locale()]),
                    )),
                ),
        )
        .record(record(PRISON, JURISDICTION, "sparse/jurisdiction.xml"))
}

fn unreadable_locale() -> TestValue {
    TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec())
}

fn parameters_declaration() -> TestStruct {
    TestStruct::new(PARAMETERS)
        .field("isFelony", TestType::Int32)
        .field("graceAllowance", TestType::Int32)
        .field("graceAllowanceCooldown", TestType::Single)
        .field("gracePeriod", TestType::Single)
        .field("graceCooloffScale", TestType::Single)
        .array("graceWarnings", TestType::Locale)
        .field("displayGraceTime", TestType::Int32)
        .field("escalatedPaymentFineMultiplier", TestType::Single)
        .field("earlyPaymentPeriod", TestType::Single)
        .field("lifetime", TestType::Single)
        .field("coolOffTime", TestType::Single)
        .field("pressChargesNotificationTime", TestType::Single)
        .field("removeTimeSeconds", TestType::Single)
        .field("felonyMerits", TestType::Int32)
        .field("ignoreIfAgainstPartyMember", TestType::Int32)
        .field("hideCrimeNotification", TestType::Int32)
        .field("hideCrimeInJournal", TestType::Int32)
        .field("futureConditions", class(CONDITION))
}

fn jurisdiction_declaration() -> TestStruct {
    TestStruct::new(JURISDICTION)
        .field("subsumptionJurisdiction", TestType::String)
        .field("name", TestType::Locale)
        .field("logoPath", TestType::String)
        .field("parentJurisdiction", reference(JURISDICTION))
        .field("respectsParentJurisdictionLaws", TestType::Bool)
        .array("infractions", class(USE_TYPE))
        .array("infractionSets", reference(SET_TYPE))
        .field("journalEntry", reference("JournalEntry"))
        .field("maxStolenGoodsPossessionSCU", TestType::Single)
        .array("prohibitedGoods", reference("EntityClassDefinition"))
        .array("prohibitedResources", reference("ResourceType"))
        .array("controlledSubstanceClasses", class(SUBSTANCE))
        .field("baseFine", TestType::Int32)
        .field("earlyPaymentPeriod", TestType::Single)
        .field("isPrison", TestType::Bool)
        .array("impoundingDefinitions", class(IMPOUND))
}

fn definition() -> TestRecord {
    record(DEFINITION, DEFINITION_TYPE, "unclassified/infraction.xml")
        .value("name", TestValue::text("@shared"))
        .value("description", TestValue::text("@shared"))
        .value(
            "triggers",
            TestValue::Array(
                [FUTURE, FIRST, FUTURE]
                    .into_iter()
                    .map(TestValue::text)
                    .collect(),
            ),
        )
        .value("defaultParameters", parameters(-1, -0.0))
}

fn set() -> TestRecord {
    record(SET, SET_TYPE, "outside/sets/fixture.xml").value(
        "infractions",
        TestValue::Array(vec![
            usage(DEFINITION, 0),
            usage(MISSING, 2),
            usage(DEFINITION, 3),
            usage(ENTITY, 4),
        ]),
    )
}

fn prison() -> TestRecord {
    record(PRISON, JURISDICTION, "selected/jurisdiction/prison.xml")
        .value("subsumptionJurisdiction", TestValue::text("Fixture.Scope"))
        .value("name", TestValue::text("@shared"))
        .value("logoPath", TestValue::text("textures/fixture.dds"))
        .value("parentJurisdiction", TestValue::Reference(PARENT))
        .value("respectsParentJurisdictionLaws", TestValue::Bool(false))
        .value("infractions", TestValue::Array(vec![usage(DEFINITION, 7)]))
        .value(
            "infractionSets",
            TestValue::Array(vec![TestValue::Reference(SET)]),
        )
        .value("journalEntry", TestValue::Reference(JOURNAL))
        .value("maxStolenGoodsPossessionSCU", TestValue::Single(-0.0))
        .value(
            "prohibitedGoods",
            TestValue::Array(vec![TestValue::Reference(ENTITY)]),
        )
        .value(
            "prohibitedResources",
            TestValue::Array(vec![TestValue::Reference(RESOURCE)]),
        )
        .value(
            "controlledSubstanceClasses",
            TestValue::Array(vec![substance()]),
        )
        .value("baseFine", TestValue::Int32(0))
        .value("earlyPaymentPeriod", TestValue::Single(2.5))
        .value("isPrison", TestValue::Bool(true))
        .value("impoundingDefinitions", TestValue::Array(vec![impound()]))
}

fn usage(guid: u128, flag: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(USE_TYPE)
            .value("definition", TestValue::Reference(guid))
            .value(
                "parameterOverrides",
                parameters(flag, f32::from_bits(0x7fc0_1072)),
            ),
    )
}

fn parameters(flag: i32, period: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(PARAMETERS)
            .value("isFelony", TestValue::Int32(flag))
            .value("graceAllowance", TestValue::Int32(0))
            .value("ignoreIfAgainstPartyMember", TestValue::Int32(-1))
            .value("gracePeriod", TestValue::Single(period))
            .value(
                "graceWarnings",
                TestValue::Array(vec![TestValue::text("@shared")]),
            )
            .value(
                "futureConditions",
                TestValue::Struct(TestInstance::new(CONDITION).value(
                    "negated",
                    TestValue::Array(vec![TestValue::Struct(
                        TestInstance::new(NEGATION)
                            .value("allow", TestValue::Bool(false))
                            .value("scale", TestValue::Single(f32::from_bits(0x7fc0_1073))),
                    )]),
                )),
            ),
    )
}

fn substance() -> TestValue {
    TestValue::Struct(
        TestInstance::new(SUBSTANCE)
            .value(
                "commodities",
                TestValue::Array(vec![TestValue::Reference(ENTITY)]),
            )
            .value(
                "resources",
                TestValue::Array(vec![TestValue::Reference(RESOURCE)]),
            )
            .value("maxPossessionSCU", TestValue::Single(0.0)),
    )
}

fn impound() -> TestValue {
    TestValue::Struct(
        TestInstance::new(IMPOUND)
            .value("name", TestValue::text("@shared"))
            .value("trigger", TestValue::text(FUTURE))
            .value("impoundingTimeSeconds", TestValue::Single(15.0))
            .value("impoundingFineUEC", TestValue::Int32(0))
            .value("ignoreIfAgainstPartyMember", TestValue::Bool(false)),
    )
}

fn class(target: &str) -> TestType {
    TestType::Class(target.to_owned())
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

fn record(guid: u128, kind: &str, path: &str) -> TestRecord {
    TestRecord::new(guid, kind, format!("{kind}.Fixture{guid}"), path)
}
