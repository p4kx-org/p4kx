// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `entity_loadout` states the notes of an Entity next to its issues, as the CLI JSON does.

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestGroup, TestLoadout, TestLoadoutEntry,
    TestRequirement, assortment_record_requiring, entity_schema,
};
use serde_json::json;

use crate::support::{Install, TestResult};

const ASSORTMENT: u128 = 0x100;
const TAG: u128 = 0x500;

fn install(requirement: Option<TestRequirement>) -> TestResult<Install> {
    let group = TestGroup::new(
        "Body",
        vec![TestLoadout::Manual(vec![TestLoadoutEntry::class(
            "Body_ItemPort",
            "Body_Red",
        )])],
    );
    let group = match requirement {
        Some(requirement) => group.requiring(requirement),
        None => group,
    };
    let forge = [
        TestEntity::new(1, "Beast", "entities/beast.xml")
            .loadout(TestLoadout::RandomGenerated(vec![ASSORTMENT])),
        TestEntity::new(2, "Body_Red", "entities/body_red.xml"),
    ]
    .iter()
    .fold(entity_schema(DataForgeBuilder::new()), |forge, entity| {
        forge.record(entity.record())
    })
    .record(assortment_record_requiring(
        ASSORTMENT,
        "beast",
        None,
        &[group],
    ));
    Install::new(vec![TestEntry::zstd("Data/Game2.dcb", forge.build())])
}

#[test]
fn a_group_that_depends_on_tags_is_a_note_of_the_entity_and_no_issue() -> TestResult {
    let install = install(Some(TestRequirement::Tags {
        required: vec![TAG],
        banned: Vec::new(),
    }))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let loadout = server.ok("entity_loadout", &json!({ "entity": "Beast" }))?;

    let entity = &loadout["entity"];
    assert_eq!(entity["issue_count"], 0, "{loadout}");
    assert_eq!(
        entity["notes"].as_array().map(Vec::len),
        Some(1),
        "{loadout}"
    );
    assert_eq!(entity["notes"][0]["kind"], "assortment_requirement");
    assert!(
        entity["notes"][0]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("the group Body")),
        "{loadout}"
    );
    assert_eq!(loadout["counts"]["attached"], 1, "the group still applies");
    Ok(())
}

#[test]
fn an_entity_without_notes_omits_the_field() -> TestResult {
    let install = install(None)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let loadout = server.ok("entity_loadout", &json!({ "entity": "Beast" }))?;

    assert!(loadout["entity"].get("notes").is_none(), "{loadout}");
    Ok(())
}
