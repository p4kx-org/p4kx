// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::TARGET;
use p4kx_testkit::{TestInstance, TestValue};

pub(crate) fn name() -> TestInstance {
    TestInstance::new("MissionPropertyValue_AIName")
        .value("randomName", TestValue::Bool(false))
        .value("randomLastName", TestValue::Bool(true))
        .value("randomNickName", TestValue::Bool(false))
        .value("characterGivenName", TestValue::text("@given"))
        .value("characterGivenLastName", TestValue::text("@family"))
        .value("characterGivenNickName", TestValue::text("@nick"))
        .value("characterNameData", TestValue::Reference(TARGET))
        .value("chanceOfNickName", TestValue::Single(0.375))
}

pub(crate) fn clue(kind: &str) -> TestInstance {
    let source = TestInstance::new(format!("Investigation_Clue{kind}"))
        .value("objectiveLongDescription", TestValue::text("@long"))
        .value("objectiveShortDescription", TestValue::text("@short"));
    match kind {
        "_CorpseScanInfo" => source
            .value("name", super::object(name()))
            .value("causeOfDeath", TestValue::text("@cause")),
        "_JournalEntry" => source.value("journalEntry", TestValue::Reference(TARGET)),
        _ => source,
    }
}

pub(crate) fn clue_item(kind: &str) -> TestInstance {
    let source = TestInstance::new(format!("Investigation_ClueItem{kind}"))
        .value("entityClass", TestValue::Reference(TARGET))
        .value("spawnTags", super::object(super::tags()))
        .value(
            "clues",
            super::array(vec![
                super::object(clue("")),
                TestValue::Null,
                super::object(clue("_CorpseScanInfo")),
                super::object(clue("_JournalEntry")),
            ]),
        );
    match kind {
        "_Corpse" => source.value("suspectRepresented", TestValue::pointer("suspect")),
        "_Datapad" => source.value("canvas", TestValue::Reference(TARGET)),
        _ => source,
    }
}

pub(crate) fn investigation() -> TestInstance {
    let suspect = TestInstance::new("Investigation_Suspect")
        .named("suspect")
        .value("name", super::object(name()))
        .value("initialSuspect", TestValue::Bool(true))
        .value("isGuilty", TestValue::Bool(false));
    TestInstance::new("MissionPropertyValue_Investigation")
        .value(
            "startingJournalEntries",
            super::array(vec![
                TestValue::Reference(TARGET),
                TestValue::Reference(super::MISSING),
                TestValue::Null,
            ]),
        )
        .value("suspects", super::array(vec![super::object(suspect)]))
        .value(
            "locations",
            super::array(vec![super::object(
                TestInstance::new("Investigation_Location").value(
                    "clueItems",
                    super::array(vec![
                        super::object(clue_item("")),
                        TestValue::Null,
                        super::object(clue_item("_Corpse")),
                        super::object(clue_item("_Datapad")),
                    ]),
                ),
            )]),
        )
}
