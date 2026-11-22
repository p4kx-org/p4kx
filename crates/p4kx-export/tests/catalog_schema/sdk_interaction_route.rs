// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::interactions::{
    InteractionSnapshot, SharedInteraction, with_interactions_from,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, Language, SourceLocation, SourceValueState};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::{Route, TestResult};

#[path = "../it/catalog_inventory_interactions_fixture.rs"]
mod fixture;

pub(in super::super) const DATASET: &str = "/configurations/interactions";

const LANGUAGES: &[Language] = &[Language::English];
const OWNER: u32 = 0;
const TAG: u32 = 2;
const DISPLAY: &str = "Fixture interaction";

pub(super) fn read() -> TestResult<Route> {
    let (_folder, archive) = open()?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    with_interactions_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(OWNER),
        CatalogLanguageSelection::Selected(LANGUAGES),
        None,
        |snapshot| {
            check_table(&snapshot, &catalog)?;
            Route::queried_rows(
                DATASET,
                snapshot.rows(),
                "both component kinds have synthetic producer parity; installed parity open",
            )
        },
    )?
}

fn check_table(snapshot: &InteractionSnapshot<'_>, catalog: &Value) -> TestResult {
    let table = json!({ "availability": snapshot.availability(), "rows": snapshot.rows() });
    assert_eq!(
        Some(&table),
        catalog.pointer(DATASET),
        "the selected owner retains the producer's complete interaction rows and their origins"
    );
    let [row, machine] = snapshot.rows() else {
        return Err("expected two distinct interaction components on the selected owner".into());
    };
    for (component, ordinal) in [(row, 1), (machine, 2)] {
        assert_eq!(
            (
                component.owner().record_index().value(),
                component.owner().component_ordinal().value(),
                component
                    .source()
                    .source_type()
                    .origin()
                    .contains_encrypted_source(),
            ),
            (Some(&OWNER), Some(&ordinal), Some(true)),
            "the original component position and encrypted payload are preserved",
        );
    }
    assert!(
        machine.state_types().is_some() && machine.interactable().is_none(),
        "the second physical component exposes typed state machine fields"
    );
    let interactable = row
        .interactable()
        .and_then(|cell| cell.value())
        .ok_or("original interactable")?;
    let Some([first, empty, _]) = interactable
        .shared_interactions()
        .value()
        .map(Vec::as_slice)
    else {
        return Err("expected the three original shared-interaction slots".into());
    };
    assert_eq!(empty.state(), SourceValueState::Null, "original null slot");
    let interaction = first.value().ok_or("original shared interaction")?;
    check_tag(snapshot, interaction)?;
    check_localization(snapshot, catalog, interaction)
}

fn check_tag(
    snapshot: &InteractionSnapshot<'_>,
    interaction: &SharedInteraction<'_>,
) -> TestResult {
    let Some([present, empty]) = interaction.query_tags().value().map(Vec::as_slice) else {
        return Err("expected the two original Tag slots".into());
    };
    assert_eq!(
        present.value().and_then(|link| link.resolved_index()),
        Some(TAG),
        "the authored query Tag reaches the exact independent physical header",
    );
    assert_eq!(
        empty.state(),
        SourceValueState::Null,
        "original null Tag slot"
    );
    let mut observed_owner = false;
    for used in snapshot.source_inventory().iter_uses() {
        if let SourceLocation::Subresource {
            namespace, indices, ..
        } = used.location()
            && namespace == "dataforgeValue"
        {
            assert_eq!(
                indices.first(),
                Some(&u64::from(OWNER)),
                "the physical interaction request observes only the selected owner body",
            );
            observed_owner = true;
        }
    }
    assert!(observed_owner, "the query reports original body evidence");
    Ok(())
}

fn check_localization(
    snapshot: &InteractionSnapshot<'_>,
    catalog: &Value,
    interaction: &SharedInteraction<'_>,
) -> TestResult {
    assert_eq!(
        snapshot.available_languages(),
        LANGUAGES,
        "requested Locale is available"
    );
    let reference = interaction
        .display_name()
        .value()
        .ok_or("authored protected key")?;
    let text = snapshot
        .game_text(reference)
        .ok_or("registered protected text")?;
    assert_eq!(
        text.translations()
            .value()
            .and_then(|languages| languages.get("en"))
            .and_then(|forms| forms.source_text.value())
            .map(Cow::as_ref),
        Some(DISPLAY),
        "the typed reference resolves to the original English text",
    );
    let producer = catalog
        .pointer("/texts/gameTexts")
        .and_then(Value::as_array)
        .ok_or("producer protected-text rows")?;
    assert!(
        producer.contains(&serde_json::to_value(text)?),
        "the exact protected text and its original origins match the producer"
    );
    Ok(())
}

#[test]
fn one_physical_owner_matches_normal_catalog() -> TestResult {
    read().map(|_| ())
}

fn open() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("interaction-fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", fixture::database().build()).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            format!("fixture_interaction={DISPLAY}\n").as_bytes(),
        ))
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    Ok((
        folder,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}
