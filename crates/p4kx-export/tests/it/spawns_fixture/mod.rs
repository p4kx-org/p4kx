// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with one contract that states ship spawns and location queries at the
//! template, handler, contract and sub contract level, and two location templates.

mod broker;
mod broker_structures;
mod npc;
mod structures;
mod template;
mod template_deep;
mod template_parts;
mod template_properties;
mod template_structures;
mod template_values;
mod unknown_conditions;

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestValue,
};

use self::npc::NpcKind;

pub(crate) const GENERATOR: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const CONTRACT: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const SUB: u128 = 0x3000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const TEMPLATE: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const LEFTOVER_TEMPLATE: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const ELSEWHERE_TEMPLATE: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const DEEP_PHASES: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const DEEP_FLOW: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const UNKNOWN_FLOW: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0006;
pub(crate) const UNKNOWN_MODIFIER: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0007;
pub(crate) const UNKNOWN_VALUE: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0008;
pub(crate) const UNKNOWN_CONDITION: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0009;
pub(crate) const UNKNOWN_CONTRACT_PROPERTY: u128 = 0x4000_0000_0000_4000_8000_0000_0000_000a;
pub(crate) const UNKNOWN_ITEM_CONDITION: u128 = 0x4000_0000_0000_4000_8000_0000_0000_000b;
pub(crate) const UNKNOWN_LOCATION_CONDITION: u128 = 0x4000_0000_0000_4000_8000_0000_0000_000c;
pub(crate) const UNKNOWN_LOCATION_AFTER_KNOWN: u128 = 0x4000_0000_0000_4000_8000_0000_0000_000d;
pub(crate) const GENERATOR_UNKNOWN_CONDITION: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const CONTRACT_UNKNOWN_CONDITION: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const BROKER_BAD_LOCATION: u128 = 0xb000_0000_0000_4000_8000_0000_0000_0006;
pub(crate) const BROKER_BAD_PROPERTY: u128 = 0xb000_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const BROKER: u128 = 0xb000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const BROKER_ELSEWHERE: u128 = 0xb000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const BROKER_LEFTOVER: u128 = 0xb000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const BROKER_BAD: u128 = 0xb000_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const BROKER_OBJECTIVE: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0010;
pub(crate) const REPUTATION_REWARD: u128 = 0xb100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const MISSION_TYPE: u128 = 0x5000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const RESOURCE: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const OBJECTIVE_A: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const OBJECTIVE_B: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const OBJECTIVE_C: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const OWNER: u128 = 0x8000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const SCOPE: u128 = 0x8000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const DATABASE: u128 = 0x9000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const TAG_ROOT: u128 = 0x9000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const TAG_HIJACKED: u128 = 0x9000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const TAG_SHIP: u128 = 0x9000_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const TAG_KILL: u128 = 0x9000_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const LOCATION_A: u128 = 0xa000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const LOCATION_B: u128 = 0xa000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const LOCATION_DEV: u128 = 0xa000_0000_0000_4000_8000_0000_0000_0003;

const TAG_PATH: &str = "libs/foundry/records/tagdatabase/tagdatabase.tagdatabase.xml";

const fn reference(guid: u128) -> TestValue {
    TestValue::Reference(guid)
}

fn text(value: &str) -> TestValue {
    TestValue::text(value)
}

const fn array(values: Vec<TestValue>) -> TestValue {
    TestValue::Array(values)
}

fn tag(guid: u128, name: &str, children: Vec<TestValue>) -> TestRecord {
    TestRecord::new(guid, "Tag", format!("Tag.{name}"), TAG_PATH)
        .value("tagName", text(name))
        .value("legacyGUID", TestValue::UInt32(u32::MAX))
        .value("children", array(children))
}

fn tag_list(tags: &[u128]) -> TestValue {
    TestValue::Struct(
        TestInstance::new("TagList")
            .value("tags", array(tags.iter().copied().map(reference).collect())),
    )
}

fn term(positive: &[u128], negative: &[u128]) -> TestInstance {
    TestInstance::new("TagSearchTerm")
        .value(
            "positiveTags",
            array(positive.iter().copied().map(reference).collect()),
        )
        .value(
            "negativeTags",
            array(negative.iter().copied().map(reference).collect()),
        )
}

fn ship_option(weight: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SpawnDescription_Ship")
            .value("concurrentAmount", TestValue::Int32(2))
            .value("includeLocationAISpawnTags", TestValue::Bool(false))
            .value("weight", TestValue::Int32(weight))
            .value("tags", tag_list(&[TAG_SHIP]))
            .value("negativeTags", tag_list(&[]))
            .value("markupTags", tag_list(&[]))
            .value("entityTags", tag_list(&[])),
    )
}

/// A ship spawn property with one group of one slot of one option of `weight`; `empty` has no
/// group at all.
fn ships(variable: &str, token: &str, weight: Option<i32>) -> TestValue {
    let groups = weight.map_or_else(Vec::new, |weight| {
        vec![TestValue::Struct(
            TestInstance::new("SpawnDescription_ShipGroup")
                .value("Name", text("Wave"))
                .value(
                    "ships",
                    array(vec![TestValue::Struct(
                        TestInstance::new("SpawnDescription_ShipOptions")
                            .value("options", array(vec![ship_option(weight)])),
                    )]),
                ),
        )]
    });
    property(
        variable,
        token,
        TestInstance::new("MissionPropertyValue_ShipSpawnDescriptions")
            .value(
                "allowedForMissionRestrictedDeliveries",
                TestValue::Bool(false),
            )
            .value("spawnDescriptions", array(groups)),
    )
}

fn location(variable: &str, token: &str, ranges: Option<(i32, i32)>) -> TestValue {
    let kind = if ranges.is_some() {
        "MissionPropertyValue_Locations"
    } else {
        "MissionPropertyValue_Location"
    };
    let mut value = TestInstance::new(kind)
        .value("logErrorOnSearchFail", TestValue::Bool(true))
        .value("resourceTags", array(vec![reference(TAG_KILL)]))
        .value(
            "matchConditions",
            array(vec![TestValue::Struct(
                TestInstance::new("DataSetMatchCondition_TagSearch")
                    .value("tagType", text("General"))
                    .value(
                        "tagSearch",
                        array(vec![TestValue::Struct(term(&[TAG_HIJACKED], &[TAG_SHIP]))]),
                    ),
            )]),
        );
    if let Some((min, max)) = ranges {
        value = value
            .value("minLocationsToFind", TestValue::Int32(min))
            .value("maxLocationsToFind", TestValue::Int32(max))
            .value("failIfMinAmountNotFound", TestValue::Bool(true));
    }
    property(variable, token, value)
}

fn property(variable: &str, token: &str, value: TestInstance) -> TestValue {
    TestValue::Struct(
        TestInstance::new("MissionProperty")
            .value("missionVariableName", text(variable))
            .value("extendedTextToken", text(token))
            .value("value", TestValue::Struct(value)),
    )
}

fn overrides(properties: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ContractParamOverrides").value("propertyOverrides", array(properties)),
    )
}

fn location_prerequisite(level: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ContractPrerequisite_LocationProperty")
            .value("locationLevelType", text(level))
            .value("propertyExtendedTextToken", text(""))
            .value("propertyVariableName", text("Place_BP")),
    )
}

fn generator() -> TestRecord {
    let sub = TestValue::Struct(
        TestInstance::new("SubContract")
            .value("id", TestValue::Guid(SUB))
            .value("debugName", text("Sub_Contract"))
            .value(
                "additionalPrerequisites",
                array(vec![location_prerequisite("Planet")]),
            )
            .value(
                "paramOverrides",
                overrides(vec![ships("Wave_BP", "", Some(3))]),
            ),
    );
    let contract = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(CONTRACT))
        .value("debugName", text("Hunt_A"))
        .value("template", reference(TEMPLATE))
        .value("missionBrokerEntry", reference(BROKER))
        .value(
            "paramOverrides",
            overrides(vec![
                ships("Wave_BP", "", Some(2)),
                ships("Empty_BP", "", None),
                location("Place_BP", "Place", Some((1, 3))),
                npc::npcs("Crew_BP", "", Some(("Squad", NpcKind::Full))),
                npc::npcs("Spare_BP", "", None),
            ]),
        )
        .value("subContracts", array(vec![sub]));
    let handler = TestInstance::new("ContractGeneratorHandler_Career")
        .value("debugName", text("Hunt_Career"))
        .value(
            "defaultAvailability",
            TestValue::Struct(TestInstance::new("ContractAvailability").value(
                "prerequisites",
                array(vec![location_prerequisite("System")]),
            )),
        )
        .value(
            "contractParams",
            overrides(vec![
                ships("Wave_BP", "", Some(1)),
                ships("Empty_BP", "", Some(1)),
                npc::npcs("Crew_BP", "", Some(("Reserve", NpcKind::Full))),
                npc::entities("Loot_BP", "", Some("Crate")),
            ]),
        )
        .value("contracts", array(vec![TestValue::Struct(contract)]));
    TestRecord::new(
        GENERATOR,
        "ContractGenerator",
        "ContractGenerator.Hunt",
        "libs/foundry/records/contracts/contractgenerator/hunt.xml",
    )
    .value("generators", array(vec![TestValue::Struct(handler)]))
}

fn location_template(guid: u128, name: &str, disabled: bool) -> TestRecord {
    TestRecord::new(
        guid,
        "MissionLocationTemplate",
        format!("MissionLocationTemplate.{name}"),
        format!("libs/foundry/records/missiondata/pu_locations/templates/{name}.xml"),
    )
    .value(
        "locationData",
        TestValue::Struct(
            TestInstance::new("MissionLocationData")
                .value("disabled", TestValue::Bool(disabled))
                .value("isSecurityNetworkHost", TestValue::Bool(false))
                .value(
                    "generalTags",
                    TestValue::Struct(
                        TestInstance::new("MissionLocationTags")
                            .value("tags", array(vec![reference(TAG_HIJACKED), reference(0)])),
                    ),
                )
                .value("producesTags", TestValue::Struct(term(&[TAG_SHIP], &[])))
                .value("consumesTags", TestValue::Struct(term(&[], &[TAG_KILL])))
                .value("aiSpawnTags", tag_list(&[TAG_SHIP]))
                .value(
                    "stringVariants",
                    TestValue::Struct(TestInstance::new("MissionStringVariants").value(
                        "variants",
                        array(vec![TestValue::Struct(
                            TestInstance::new("MissionStringVariant")
                                .value("tag", reference(TAG_HIJACKED))
                                .value("string", TestValue::text("@place_name")),
                        )]),
                    )),
                ),
        ),
    )
}

fn database() -> DataForgeBuilder {
    let builder = structures::structures(DataForgeBuilder::new().version(8)).instance(
        "outside",
        TestInstance::new("ObjectiveToken").value("id", TestValue::Guid(OBJECTIVE_C)),
    );
    [
        TestRecord::new(DATABASE, "TagDatabase", "TagDatabase.TagDatabase", TAG_PATH).value(
            "tags",
            array(vec![
                reference(TAG_ROOT),
                reference(TAG_SHIP),
                reference(TAG_KILL),
            ]),
        ),
        tag(TAG_ROOT, "Missions", vec![reference(TAG_HIJACKED)]),
        tag(TAG_HIJACKED, "Hijacked", vec![]),
        tag(TAG_SHIP, "ShipModel", vec![]),
        tag(TAG_KILL, "Kill", vec![]),
        generator(),
        template::template(),
        template::leftover(),
        template_parts::owner(),
        template::elsewhere(),
        template_deep::deep_phases(),
        template_deep::deep_flow(),
        template_deep::unknown_flow(),
        template_deep::unknown_modifier(),
        template_deep::unknown_value(),
        template_deep::unknown_condition(),
        template_deep::unknown_contract_property(),
        template_deep::unknown_item_condition(),
        unknown_conditions::template(),
        unknown_conditions::template_after_known(),
        unknown_conditions::generator(),
        broker::bad_location_condition(),
        broker::broker(),
        broker::elsewhere(),
        broker::leftover(),
        broker::bad(),
        broker::bad_property(),
        broker::reward_record(),
        location_template(LOCATION_A, "a_hijacked", false),
        location_template(LOCATION_B, "b_hijacked", true),
        location_template(LOCATION_DEV, "dev_temp_hijacked", false),
    ]
    .into_iter()
    .fold(builder, DataForgeBuilder::record)
}

fn english() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\english\\global.ini",
        GlobalIniBuilder::new()
            .text("place_name", "Hijacked Ship")
            .build(),
    )
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .entry(english())
        .write_to(&path)?;
    Ok(path)
}
