// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};

use crate::mission_values_fixture::{self as fixture, Owner, TestResult};

#[path = "mission_values_canonical_investigation.rs"]
mod canonical;

#[test]
fn investigation_keeps_all_clue_variants_and_owner_local_non_property_weak_links() -> TestResult {
    for owner in [Owner::Generator, Owner::Template, Owner::Broker] {
        let builder = fixture::database(
            vec![fixture::property("Case", fixture::investigation())],
            owner,
        );
        if matches!(owner, Owner::Template) {
            crate::mission_template_original::owner_is_first(&builder)?;
        }
        let document = fixture::build(&builder, CatalogPublicationMode::Normal)?;
        if !matches!(owner, Owner::Generator) {
            canonical::verify(&document, owner)?;
            continue;
        }
        let case = fixture::generator_value(&document, "Case")?;
        verify_investigation_fields(case)?;
        verify_items(case)?;
    }
    Ok(())
}

fn verify_investigation_fields(case: &Value) -> TestResult {
    assert_eq!(
        *fixture::field(case, "/type")?,
        "investigation",
        "Investigation retains its typed source payload"
    );
    assert_eq!(
        *fixture::field(case, "/startingJournalEntries/1/status")?,
        "dangling",
        "journal targets retain unresolved identity"
    );
    assert_eq!(
        *fixture::field(case, "/startingJournalEntries/2")?,
        Value::Null,
        "nullable Reference order is retained"
    );
    assert_eq!(
        *fixture::field(case, "/suspects/0/initialSuspect")?,
        true,
        "the initial-suspect flag is source data"
    );
    assert_eq!(
        *fixture::field(case, "/suspects/0/isGuilty")?,
        false,
        "false is preserved"
    );
    verify_name(fixture::field(case, "/suspects/0/name")?)?;
    Ok(())
}

fn verify_items(case: &Value) -> TestResult {
    let items = case
        .pointer("/locations/0/clueItems")
        .and_then(Value::as_array)
        .ok_or("clue items missing")?;
    assert_eq!(items.len(), 4, "all item positions survive");
    assert_eq!(
        items.first().and_then(|item| item.pointer("/details/type")),
        Some(&json!("base")),
        "the base item is supported"
    );
    assert_eq!(
        items.get(1),
        Some(&Value::Null),
        "a null clue-item pointer is retained"
    );
    let corpse = items.get(2).ok_or("corpse missing")?;
    assert_eq!(
        *fixture::field(corpse, "/details/type")?,
        "corpse",
        "the concrete corpse item is typed"
    );
    assert_eq!(
        *fixture::field(corpse, "/details/suspectRepresented/status")?,
        "resolved",
        "a suspect is resolved without mission-variable interpretation"
    );
    assert_eq!(
        *fixture::field(corpse, "/details/suspectRepresented/targetType")?,
        "Investigation_Suspect",
        "the raw target type survives"
    );
    assert!(
        fixture::field(corpse, "/details/suspectRepresented/location")?
            .as_str()
            .is_some_and(|path| path.ends_with("suspects[0]")),
        "the canonical owner-local location is retained"
    );
    assert_eq!(
        items
            .get(3)
            .and_then(|item| item.pointer("/details/canvas/status")),
        Some(&json!("present")),
        "the datapad canvas is exported"
    );
    verify_clues(corpse)?;
    Ok(())
}

fn verify_name(name: &Value) -> TestResult {
    assert_eq!(
        *fixture::field(name, "/randomName")?,
        false,
        "the authored name flags survive"
    );
    assert_eq!(
        *fixture::field(name, "/randomLastName")?,
        true,
        "the authored surname flag survives"
    );
    assert_eq!(
        *fixture::field(name, "/randomNickName")?,
        false,
        "the authored nickname flag survives"
    );
    assert_eq!(
        *fixture::field(name, "/givenName/key")?,
        "@given",
        "localisation keys survive"
    );
    assert_eq!(
        *fixture::field(name, "/givenLastName/key")?,
        "@family",
        "surname keys survive"
    );
    assert_eq!(
        *fixture::field(name, "/givenNickName/key")?,
        "@nick",
        "nickname keys survive"
    );
    assert_eq!(
        *fixture::field(name, "/chanceOfNickName")?,
        0.375,
        "fractional chance is exact"
    );
    assert!(
        fixture::field(name, "/nameDataGuid")?.is_string(),
        "the original name-data target survives"
    );
    Ok(())
}

fn verify_clues(item: &Value) -> TestResult {
    let clues = fixture::field(item, "/clues")?
        .as_array()
        .ok_or("clues missing")?;
    assert_eq!(clues.len(), 4, "all clue declarations survive");
    assert_eq!(
        clues.first().and_then(|clue| clue.pointer("/details/type")),
        Some(&json!("base")),
        "the field-bearing base clue survives"
    );
    assert_eq!(
        clues.get(1),
        Some(&Value::Null),
        "a null clue pointer is retained"
    );
    let scan = clues.get(2).ok_or("scan clue missing")?;
    assert_eq!(
        *fixture::field(scan, "/objectiveLongDescription/key")?,
        "@long",
        "the common long text survives subtyping"
    );
    assert_eq!(
        *fixture::field(scan, "/objectiveShortDescription/key")?,
        "@short",
        "the common short text survives subtyping"
    );
    assert_eq!(
        *fixture::field(scan, "/details/causeOfDeath/key")?,
        "@cause",
        "the corpse text survives"
    );
    verify_name(fixture::field(scan, "/details/name")?)?;
    assert_eq!(
        clues
            .get(3)
            .and_then(|clue| clue.pointer("/details/journalEntry/status")),
        Some(&json!("present")),
        "the journal clue target survives"
    );
    Ok(())
}
