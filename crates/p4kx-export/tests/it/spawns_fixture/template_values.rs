// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The contract-level properties of the fixture's template whose values are mission items, names,
//! combined data set entries, rewards and races.

use p4kx_testkit::{TestInstance, TestStruct, TestType, TestValue};

use super::template_parts::object;
use super::{OWNER, TAG_HIJACKED, TAG_SHIP, array, npc, property, reference, term, text};

fn value(name: &str) -> TestStruct {
    TestStruct::new(name).parent("MissionPropertyValue")
}

/// The structs of the values.
pub(super) fn structs() -> Vec<TestStruct> {
    vec![
        value("MissionPropertyValue_MissionItem")
            .array(
                "matchConditions",
                TestType::StrongPointer("DataSetMatchCondition".into()),
            )
            .field("debugForceChosenItem", TestType::Reference)
            .field("minItemsToFind", TestType::Int32)
            .field("maxItemsToFind", TestType::Int32),
        TestStruct::new("DataSetMatchCondition_SpecificItemsDef")
            .parent("DataSetMatchCondition")
            .array("items", TestType::Reference),
        value("MissionPropertyValue_CombinedDataSetEntries").array(
            "dataSetEntryProperties",
            TestType::WeakPointer("MissionProperty".into()),
        ),
        value("MissionPropertyValue_Reward").field(
            "rewardDef",
            TestType::Class("MissionRewardDefinition".into()),
        ),
        TestStruct::new("MissionRewardDefinition")
            .field("currencyType", TestType::String)
            .field("reward", TestType::Int32)
            .field("max", TestType::Int32)
            .field("plusBonuses", TestType::Bool)
            .field("reputationBonus", TestType::Reference),
        value("MissionPropertyValue_TimeTrialRace"),
        value("MissionPropertyValue_StarMapLocation"),
    ]
}

/// The properties, in this order: an item, a name, combined entries, a reward, a race and a star
/// map location.
pub(super) fn properties() -> Vec<TestValue> {
    let item = TestInstance::new("MissionPropertyValue_MissionItem")
        .value(
            "matchConditions",
            array(vec![
                object(
                    TestInstance::new("DataSetMatchCondition_TagSearch")
                        .value("tagType", text("General"))
                        .value(
                            "tagSearch",
                            array(vec![object(term(&[TAG_HIJACKED], &[TAG_SHIP]))]),
                        ),
                ),
                object(
                    TestInstance::new("DataSetMatchCondition_SpecificItemsDef")
                        .value("items", array(vec![reference(OWNER)])),
                ),
            ]),
        )
        .value("debugForceChosenItem", reference(OWNER))
        .value("minItemsToFind", TestValue::Int32(1))
        .value("maxItemsToFind", TestValue::Int32(3));
    let combined = TestInstance::new("MissionPropertyValue_CombinedDataSetEntries").value(
        "dataSetEntryProperties",
        array(vec![
            TestValue::pointer("prop_place"),
            TestValue::pointer("prop_home"),
        ]),
    );
    let reward = TestInstance::new("MissionPropertyValue_Reward").value(
        "rewardDef",
        object(
            TestInstance::new("MissionRewardDefinition")
                .value("currencyType", text("UEC"))
                .value("reward", TestValue::Int32(900))
                .value("max", TestValue::Int32(1_200))
                .value("plusBonuses", TestValue::Bool(true))
                .value("reputationBonus", reference(OWNER)),
        ),
    );
    vec![
        property("Item_BP", "", item),
        npc::name("Name_BP"),
        property("Combined_BP", "", combined),
        property("Pay_BP", "", reward),
        property(
            "Race_BP",
            "",
            TestInstance::new("MissionPropertyValue_TimeTrialRace"),
        ),
        property(
            "Map_BP",
            "",
            TestInstance::new("MissionPropertyValue_StarMapLocation"),
        ),
    ]
}
