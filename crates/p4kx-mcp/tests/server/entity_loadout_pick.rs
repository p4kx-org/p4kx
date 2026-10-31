// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `entity_loadout` with `loadout_pick` chooses the alternative of a group of a generated Loadout.

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, assortment_record,
    entity_schema,
};
use serde_json::{Value, json};

use crate::support::{Install, TestResult};

const ASSORTMENT: u128 = 0x100;

fn body(class: &str) -> TestLoadout {
    TestLoadout::Manual(vec![TestLoadoutEntry::class("Body_ItemPort", class)])
}

fn install() -> TestResult<Install> {
    let forge = [
        TestEntity::new(1, "Beast", "entities/beast.xml")
            .loadout(TestLoadout::RandomGenerated(vec![ASSORTMENT])),
        TestEntity::new(2, "Body_Red", "entities/body_red.xml"),
        TestEntity::new(3, "Body_Blue", "entities/body_blue.xml"),
    ]
    .iter()
    .fold(entity_schema(DataForgeBuilder::new()), |forge, entity| {
        forge.record(entity.record())
    })
    .record(assortment_record(
        ASSORTMENT,
        "beast",
        &[
            ("Body", vec![body("Body_Red"), body("Body_Blue")]),
            (
                "Eggs",
                vec![TestLoadout::Manual(vec![TestLoadoutEntry::class(
                    "Egg_ItemPort",
                    "Body_Red",
                )])],
            ),
        ],
    ));
    Install::new(vec![TestEntry::zstd("Data/Game2.dcb", forge.build())])
}

fn body_class(loadout: &Value) -> Option<&str> {
    let port = loadout["ports"]
        .as_array()?
        .iter()
        .find(|port| port["path"] == "Body_ItemPort")?;
    port.get("entity")?.get("class_name")?.as_str()
}

#[test]
fn the_first_alternative_is_attached_without_a_pick() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let loadout = server.ok("entity_loadout", &json!({ "entity": "Beast" }))?;

    assert_eq!(body_class(&loadout), Some("Body_Red"), "{loadout}");
    Ok(())
}

#[test]
fn a_pick_attaches_another_alternative_and_is_part_of_the_cache_key() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let picked = server.ok(
        "entity_loadout",
        &json!({ "entity": "Beast", "loadout_pick": ["Body=1"] }),
    )?;
    let default = server.ok("entity_loadout", &json!({ "entity": "Beast" }))?;

    assert_eq!(body_class(&picked), Some("Body_Blue"), "{picked}");
    assert_eq!(body_class(&default), Some("Body_Red"), "{default}");
    Ok(())
}

#[test]
fn an_index_beyond_the_group_is_an_issue_of_the_entity() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let loadout = server.ok(
        "entity_loadout",
        &json!({ "entity": "Beast", "loadout_pick": ["Body=9"] }),
    )?;

    assert_eq!(loadout["entity"]["issues"][0]["kind"], "assortment");
    assert_eq!(body_class(&loadout), Some("Body_Red"), "{loadout}");
    Ok(())
}

#[test]
fn a_malformed_pick_is_an_error() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let error = server.error(
        "entity_loadout",
        &json!({ "entity": "Beast", "loadout_pick": ["Body"] }),
    )?;

    assert!(error.contains("Body"), "{error}");
    Ok(())
}

#[test]
fn a_named_group_the_loadout_lacks_is_an_issue_and_a_bare_index_beyond_all_is_not() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let typo = server.ok(
        "entity_loadout",
        &json!({ "entity": "Beast", "loadout_pick": ["Bodyy=1"] }),
    )?;
    let bare = server.ok(
        "entity_loadout",
        &json!({ "entity": "Beast", "loadout_pick": ["9"] }),
    )?;

    assert_eq!(typo["entity"]["issues"][0]["kind"], "assortment");
    assert!(
        typo["entity"]["issues"][0]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("Bodyy=1")),
        "{typo}"
    );
    assert_eq!(body_class(&typo), Some("Body_Red"));
    assert_eq!(bare["entity"]["issue_count"], 0, "{bare}");
    Ok(())
}

#[test]
fn a_record_that_is_no_entity_stays_a_loadout_error_with_picks() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let assortment = "00000000-0000-0000-0000-000000000100";

    let plain = server.error("entity_loadout", &json!({ "entity": assortment }))?;
    let picked = server.error(
        "entity_loadout",
        &json!({ "entity": assortment, "loadout_pick": ["Body=1"] }),
    )?;

    assert_eq!(plain, picked, "the pick does not change the error");
    assert!(!picked.contains("picks are not valid"), "{picked}");
    Ok(())
}

#[test]
fn a_cursor_of_other_picks_is_rejected() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.ok(
        "entity_loadout",
        &json!({ "entity": "Beast", "limit": 1, "loadout_pick": ["Body=1", "Eggs=0"] }),
    )?;
    let cursor = first["next_cursor"].as_str().ok_or("no next page")?;

    let same_picks_other_form = server.ok(
        "entity_loadout",
        &json!({ "entity": "Beast", "limit": 1, "cursor": cursor,
                 "loadout_pick": ["eggs=0, body=1"] }),
    )?;
    let error = server.error(
        "entity_loadout",
        &json!({ "entity": "Beast", "limit": 1, "cursor": cursor, "loadout_pick": ["Body=0"] }),
    )?;

    assert!(same_picks_other_form["ports"].is_array());
    assert!(error.contains("cursor"), "{error}");
    Ok(())
}

#[test]
fn the_number_and_length_of_picks_are_capped() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let many: Vec<String> = (0..65).map(|index| format!("G{index}=0")).collect();
    let long = format!("{}=1", "g".repeat(300));

    let too_many = server.error(
        "entity_loadout",
        &json!({ "entity": "Beast", "loadout_pick": many }),
    )?;
    let too_long = server.error(
        "entity_loadout",
        &json!({ "entity": "Beast", "loadout_pick": [long] }),
    )?;

    assert!(too_many.contains("at most 64"), "{too_many}");
    assert!(too_long.contains("loadout_pick"), "{too_long}");
    Ok(())
}
