// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Language};
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::text_row;
use crate::health_origin_assertions as proof;
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
        verify_investigation_fields(&document, case)?;
        verify_items(&document, case)?;
    }
    Ok(())
}

fn verify_investigation_fields(catalog: &Value, case: &Value) -> TestResult {
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
    verify_name(catalog, fixture::field(case, "/suspects/0/name")?)?;
    Ok(())
}

fn verify_items(catalog: &Value, case: &Value) -> TestResult {
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
    verify_clues(catalog, corpse)?;
    Ok(())
}

fn verify_name(catalog: &Value, name: &Value) -> TestResult {
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
    for (field, raw_field, key) in [
        ("/givenName", "characterGivenName", "@given"),
        ("/givenLastName", "characterGivenLastName", "@family"),
        ("/givenNickName", "characterGivenNickName", "@nick"),
    ] {
        assert_eq!(
            name.get("source")
                .and_then(|source| source.get(raw_field))
                .and_then(Value::as_str),
            Some(key),
            "each raw name token remains available"
        );
        check_game_text_key(catalog, fixture::field(name, field)?, key)?;
    }
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

fn verify_clues(catalog: &Value, item: &Value) -> TestResult {
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
    for (path, key) in [
        ("/objectiveLongDescription", "@long"),
        ("/objectiveShortDescription", "@short"),
        ("/details/causeOfDeath", "@cause"),
    ] {
        check_game_text_key(catalog, fixture::field(scan, path)?, key)?;
    }
    verify_name(catalog, fixture::field(scan, "/details/name")?)?;
    assert_eq!(
        clues
            .get(3)
            .and_then(|clue| clue.pointer("/details/journalEntry/status")),
        Some(&json!("present")),
        "the journal clue target survives"
    );
    Ok(())
}

#[test]
fn clue_texts_preserve_literal_empty_and_placeholder_locale_tokens() -> TestResult {
    let value = fixture::localized_probe(["literal", "", "@LOC_PLACEHOLDER"]);
    let database = fixture::database(vec![fixture::property("Case", value)], Owner::Generator);
    let english = TestEntry::stored(
        Language::English.global_ini_path(),
        b"literal=Literal lookup\nLOC_PLACEHOLDER=Must stay hidden\n",
    );
    let catalog =
        fixture::build_with_entries(&database, CatalogPublicationMode::Normal, [english])?;
    let investigation = fixture::generator_value(&catalog, "Case")?;
    let clue = fixture::field(investigation, "/locations/0/clueItems/0/clues/0")?;
    let mut positions = std::collections::BTreeSet::new();
    for (path, key, display) in [
        (
            "/objectiveLongDescription",
            "literal",
            Some("Literal lookup"),
        ),
        ("/objectiveShortDescription", "", None),
        ("/details/causeOfDeath", "@LOC_PLACEHOLDER", None),
    ] {
        let cell = fixture::field(clue, path)?;
        let registry = check_game_text_key(&catalog, cell, key)?;
        let translation = fixture::field(registry, "/translations/value/en/displayText/value")?;
        assert_eq!(
            translation.as_str(),
            display,
            "{path} has its original lookup policy"
        );
        let position = game_text_payload(&catalog, cell)?
            .pointer("/1")
            .and_then(Value::as_str)
            .ok_or("original Locale value position")?;
        positions.insert(position);
    }
    assert_eq!(
        positions.len(),
        3,
        "each original Locale field retains its own value position"
    );
    Ok(())
}

#[test]
fn conflicting_clue_texts_leave_the_other_property_sets_and_no_orphan_text() -> TestResult {
    use crate::mission_property_sets_fixture::{generator, parameters};

    let retained = || fixture::property("Case", fixture::localized_probe(["@kept", "", ""]));
    let rejected = fixture::property("Case", fixture::localized_probe(["@discarded", "", ""]));
    let database = fixture::database(vec![retained()], Owner::Generator).record(generator([
        parameters(vec![rejected]),
        parameters(vec![retained()]),
        parameters(vec![retained()]),
    ]));
    let catalog = fixture::build(&database, CatalogPublicationMode::DiagnosticPartial)?;
    let sets = proof::rows(&catalog, "/missionPropertySets")?;
    assert_eq!(
        sets.len(),
        3,
        "only the conflicting handler set is quarantined"
    );
    for set in sets {
        assert_ne!(
            set["level"], "handler",
            "the ambiguous handler cannot publish"
        );
        let cell = fixture::field(
            set,
            "/properties/0/value/locations/0/clueItems/0/clues/0/objectiveLongDescription",
        )?;
        check_game_text_key(&catalog, cell, "@kept")?;
    }
    assert!(
        proof::rows(&catalog, "/texts/gameTexts")?
            .iter()
            .all(|row| row.pointer("/key/value").and_then(Value::as_str) != Some("@discarded")),
        "the rejected original key cannot register an orphan protected text",
    );
    Ok(())
}

fn check_game_text_key<'catalog>(
    catalog: &'catalog Value,
    cell: &Value,
    key: &str,
) -> Result<&'catalog Value, Box<dyn std::error::Error>> {
    assert_eq!(
        cell["state"], "present",
        "the authored token has its own protected reference"
    );
    let registry = text_row(catalog, "gameTexts", &cell["value"])?;
    assert_eq!(
        registry.pointer("/key/value").and_then(Value::as_str),
        Some(key),
        "the registry keeps the original Locale token",
    );
    assert_eq!(
        cell["containsEncryptedSource"], false,
        "the name or clue field comes from the clear synthetic Archive entry",
    );
    assert_eq!(
        cell["sourceSetComplete"], true,
        "the actual key field was observed",
    );
    let source = game_text_payload(catalog, cell)?;
    assert!(
        proof::uses(catalog, &registry["key"])?
            .iter()
            .any(|used| used["role"] == "payload"
                && used.pointer("/location/indices") == Some(source)),
        "the protected key uses the actual authored field"
    );
    Ok(registry)
}

fn game_text_payload<'catalog>(
    catalog: &'catalog Value,
    cell: &Value,
) -> Result<&'catalog Value, Box<dyn std::error::Error>> {
    proof::uses(catalog, cell)?
        .into_iter()
        .find(|used| {
            used["role"] == "payload"
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
        })
        .and_then(|used| used.pointer("/location/indices"))
        .ok_or_else(|| "protected text requires an original Locale payload".into())
}
