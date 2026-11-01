// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The objective properties of the fixture's contract template.

use p4kx_testkit::{TestInstance, TestValue};

use super::npc::{self, NpcKind};
use super::template_parts::{locale, object};
use super::{TAG_KILL, array, location, property, reference, ships, text};

fn objective_property(kind: &str, property: TestValue) -> TestInstance {
    TestInstance::new(format!("ObjectiveProperty_{kind}")).value("property", property)
}

fn embedded(property: TestValue) -> TestValue {
    object(objective_property("Embedded", property))
}

fn string_hash() -> TestInstance {
    TestInstance::new("MissionPropertyValue_StringHash").value(
        "options",
        array(vec![object(
            TestInstance::new("MissionPropertyValueOption_StringHash")
                .value("textId", locale("@place_name"))
                .value("value", text("calm"))
                .value("weighting", TestValue::Int32(3))
                .value(
                    "dependentProperties",
                    array(vec![TestValue::pointer("prop_home")]),
                ),
        )]),
    )
}

fn integer() -> TestInstance {
    TestInstance::new("MissionPropertyValue_Integer").value(
        "options",
        array(vec![object(
            TestInstance::new("MissionPropertyValueOption_Number")
                .value("textId", locale("@LOC_UNINITIALIZED"))
                .value("value", TestValue::Single(5.0))
                .value("variation", TestValue::Single(1.0))
                .value("weighting", TestValue::Int32(2)),
        )]),
    )
}

fn tags() -> TestInstance {
    TestInstance::new("MissionPropertyValue_Tags")
        .value(
            "tags",
            object(TestInstance::new("TagList").value("tags", array(vec![reference(TAG_KILL)]))),
        )
        .value("negativeTags", object(TestInstance::new("TagList")))
}

/// The properties of the first objective: one of every kind of value the catalog reads, a
/// referenced property and an output that the input of its phase takes.
pub(super) fn properties() -> Vec<TestValue> {
    let referenced = TestInstance::new("ObjectiveProperty_Referenced")
        .value("missionVariableName", text("Cargo_BP"))
        .value("extendedTextToken", text("Cargo"))
        .value("property", TestValue::pointer("prop_place"));
    let output = objective_property(
        "Output",
        property(
            "Out_BP",
            "out",
            TestInstance::new("MissionPropertyValue_Object"),
        ),
    )
    .named("out_prop");
    vec![
        object(referenced),
        object(output),
        embedded(property(
            "Flag_BP",
            "",
            TestInstance::new("MissionPropertyValue_Boolean").value("value", TestValue::Bool(true)),
        )),
        embedded(property("Mood_BP", "", string_hash())),
        embedded(property("Count_BP", "", integer())),
        embedded(property("Tags_BP", "", tags())),
        embedded(npc::npcs("Crew_BP", "", Some(("Squad", NpcKind::Full)))),
        embedded(ships("Embedded_Ships", "", Some(7))),
        embedded(location("Embedded_Loc", "", None)),
        embedded(npc::entities("Loot_BP", "", Some("Crate"))),
        embedded(npc::organization("Giver_BP", true)),
    ]
}

/// The input property of the phase of the first objective, which takes the output `Out_BP`.
pub(super) fn input_property() -> TestValue {
    object(
        objective_property(
            "Input",
            property(
                "In_BP",
                "",
                TestInstance::new("MissionPropertyValue_DeliveryOrder"),
            ),
        )
        .value("linkedOutput", TestValue::pointer("out_prop")),
    )
}
