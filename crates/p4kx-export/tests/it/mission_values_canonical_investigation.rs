// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::loot_v3_graph as raw;
use crate::mission_canonical_text as canonical;
use crate::mission_template_graph::{self as source, Property};
use crate::mission_values_fixture::{Owner, TestResult};

const TARGET: &str = "79000000-0000-4000-8000-000000000001";
const MISSING: &str = "79000000-0000-4000-8000-000000000002";
type TextInputs<'catalog> = Vec<(&'catalog Value, &'static str)>;

pub(super) fn verify(catalog: &Value, owner: Owner) -> TestResult {
    let property = owner.property(catalog, "Case")?;
    property.kind("investigation")?;
    raw::structure(property.node, "MissionPropertyValue_Investigation")?;
    let journal = property.field(property.node, "startingJournalEntries")?;
    assert_eq!(
        raw::elements(journal)?.len(),
        3,
        "all authored journal target slots survive"
    );
    source::reference(property.element(journal, 0)?, TARGET, Some(2))?;
    source::reference(property.element(journal, 1)?, MISSING, None)?;
    raw::source_null(property.element(journal, 2)?);
    let suspects = property.field(property.node, "suspects")?;
    assert_eq!(
        raw::elements(suspects)?.len(),
        1,
        "the original suspect array is complete"
    );
    let suspect = property.element(suspects, 0)?;
    raw::structure(suspect, "Investigation_Suspect")?;
    assert_eq!(
        source::scalar(property.field(suspect, "initialSuspect")?, "boolean")?,
        &json!(true),
        "the original initial-suspect flag survives"
    );
    assert_eq!(
        source::scalar(property.field(suspect, "isGuilty")?, "boolean")?,
        &json!(false),
        "an authored false flag is not absence"
    );
    let mut texts = Vec::new();
    name(&property, property.field(suspect, "name")?, &mut texts)?;
    items(&property, &mut texts)?;
    for (node, key) in texts {
        canonical::key(&property, catalog, node, key, "lookup", Some(key))?;
    }
    Ok(())
}

fn items<'catalog>(property: &Property<'catalog>, texts: &mut TextInputs<'catalog>) -> TestResult {
    let location = property.element(property.field(property.node, "locations")?, 0)?;
    let items = property.field(location, "clueItems")?;
    assert_eq!(
        raw::elements(items)?.len(),
        4,
        "all original clue-item positions survive"
    );
    raw::structure(property.element(items, 0)?, "Investigation_ClueItem")?;
    raw::source_null(property.element(items, 1)?);
    let corpse = property.element(items, 2)?;
    raw::structure(corpse, "Investigation_ClueItem_Corpse")?;
    let represented = property.field(corpse, "suspectRepresented")?;
    let target = source::scalar(represented, "weakReference")?;
    assert_eq!(
        raw::present(
            target
                .get("targetRecordIndex")
                .ok_or("actual owning suspect Record")?
        )?,
        &json!(0),
        "the represented suspect genuinely resolves inside its independently checked physical owner"
    );
    assert_eq!(
        raw::present(
            target
                .pointer("/target/sourceType")
                .ok_or("original suspect type")?
        )?,
        &json!("Investigation_Suspect"),
        "a non-property weak target retains its own type"
    );
    assert!(
        raw::present(
            target
                .pointer("/target/location")
                .ok_or("original suspect instance location")?
        )?
        .as_str()
        .is_some_and(|path| path.ends_with("suspects[0]")),
        "the exact owner-local suspect location is retained"
    );
    assert_eq!(
        property.binding(represented)?.get("missionPropertyStatus"),
        Some(&Value::Null),
        "the resolved suspect does not acquire a fabricated mission-variable interpretation"
    );
    let datapad = property.element(items, 3)?;
    raw::structure(datapad, "Investigation_ClueItem_Datapad")?;
    source::reference(property.field(datapad, "canvas")?, TARGET, Some(2))?;
    clues(property, corpse, texts)
}

fn clues<'catalog>(
    property: &Property<'catalog>,
    corpse: &Value,
    texts: &mut TextInputs<'catalog>,
) -> TestResult {
    let clues = property.field(corpse, "clues")?;
    assert_eq!(
        raw::elements(clues)?.len(),
        4,
        "all original clue declarations remain in order"
    );
    raw::structure(property.element(clues, 0)?, "Investigation_Clue")?;
    raw::source_null(property.element(clues, 1)?);
    let scan = property.element(clues, 2)?;
    raw::structure(scan, "Investigation_Clue_CorpseScanInfo")?;
    for (field, expected) in [
        ("objectiveLongDescription", "@long"),
        ("objectiveShortDescription", "@short"),
        ("causeOfDeath", "@cause"),
    ] {
        let node = property.field(scan, field)?;
        assert_eq!(
            source::text(node)?,
            expected,
            "the original common and derived clue text survives subtyping"
        );
        texts.push((node, expected));
    }
    name(property, property.field(scan, "name")?, texts)?;
    let journal = property.element(clues, 3)?;
    raw::structure(journal, "Investigation_Clue_JournalEntry")?;
    source::reference(property.field(journal, "journalEntry")?, TARGET, Some(2))?;
    Ok(())
}

fn name<'catalog>(
    property: &Property<'catalog>,
    name: &Value,
    texts: &mut TextInputs<'catalog>,
) -> TestResult {
    raw::structure(name, "MissionPropertyValue_AIName")?;
    for (field, expected) in [
        ("randomName", false),
        ("randomLastName", true),
        ("randomNickName", false),
    ] {
        assert_eq!(
            source::scalar(property.field(name, field)?, "boolean")?,
            &json!(expected),
            "the original authored name flags remain exact"
        );
    }
    for (field, expected) in [
        ("characterGivenName", "@given"),
        ("characterGivenLastName", "@family"),
        ("characterGivenNickName", "@nick"),
    ] {
        let node = property.field(name, field)?;
        assert_eq!(
            source::text(node)?,
            expected,
            "the original name keys are preserved before contextual lookup"
        );
        texts.push((node, expected));
    }
    canonical::single(property.field(name, "chanceOfNickName")?, 0.375)?;
    source::reference(property.field(name, "characterNameData")?, TARGET, Some(2))?;
    Ok(())
}
