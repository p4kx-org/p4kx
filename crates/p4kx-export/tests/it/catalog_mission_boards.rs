// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

#[path = "mission_board_fixture.rs"]
mod fixture;

const BOARDS: &str = "/configurations/missionBoards/rows";

#[test]
fn mission_board_owners_outside_items_preserve_original_component_slots() -> TestResult {
    let catalog = export(DcbOrigin::Encrypted, None)?;
    let rows = at(&catalog, BOARDS)?.as_array().ok_or("board rows")?;
    assert_eq!(
        rows.len(),
        4,
        "every authored provider at the selected owner"
    );
    for (row, ordinal) in rows.iter().zip([1, 2, 4, 5]) {
        assert_eq!(
            at(row, "/owner/ownerGuid/value")?,
            &json!(guid(fixture::OWNER))
        );
        assert_eq!(at(row, "/owner/componentOrdinal/value")?, ordinal);
        assert_eq!(
            at(row, "/owner/componentOrdinal/containsEncryptedSource")?,
            true
        );
    }
    let career = at(&catalog, &format!("{BOARDS}/0/boardType/value/details"))?;
    assert_eq!(at(career, "/type")?, "career");
    assert_eq!(
        at(career, "/guildFactionReputation/value/guid/value")?,
        &json!(guid(fixture::SECOND)),
    );
    let first = at(career, "/factions/value/0")?;
    let repeated = at(career, "/factions/value/3")?;
    assert_eq!(
        at(first, "/value/guid/value")?,
        &json!(guid(fixture::FIRST))
    );
    assert_eq!(
        at(repeated, "/value/guid/value")?,
        at(first, "/value/guid/value")?
    );
    assert_eq!(at(first, "/containsEncryptedSource")?, true);
    assert_eq!(at(career, "/factions/value/1/state")?, "null");
    assert_eq!(at(career, "/factions/value/2/value/status")?, "dangling");
    assert_eq!(
        at(
            &catalog,
            &format!("{BOARDS}/1/boardType/value/details/type")
        )?,
        "notice"
    );
    assert_eq!(
        at(
            &catalog,
            &format!("{BOARDS}/2/boardType/value/details/type")
        )?,
        "authored"
    );
    assert_eq!(
        at(&catalog, &format!("{BOARDS}/3/boardType/state"))?,
        "null"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = validator.validate(&catalog) {
        return Err(format!("{}: {}", error.instance_path(), error).into());
    }
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

#[test]
fn mission_board_owner_filter_keeps_the_independent_table_availability() -> TestResult {
    let filter = Filter::new("*/factionreputation/*");
    let catalog = export(DcbOrigin::Plain, Some(&filter))?;
    assert_eq!(
        at(&catalog, "/configurations/missionBoards/availability")?,
        "available"
    );
    assert_eq!(
        at(&catalog, BOARDS)?,
        &json!([]),
        "the owner is outside the filter"
    );
    Ok(())
}

fn export(origin: DcbOrigin, filter: Option<&Filter>) -> TestResult<Value> {
    let (_folder, archive) = common::open(&fixture::database(), origin)?;
    let mut options = common::options();
    options.filter = filter;
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}
