// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, GlobalIniBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::ReducedError;
use super::faction_reputations::{self, Dependencies, PATH};
use super::record_targets::RecordTargets;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::build::original_input::test_support::TestResult;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ReputationRows,
    ReputationSelection, ReputationSnapshot,
};

#[path = "../../tests/it/reputation_faction_owner_fixture.rs"]
mod fixture;

#[test]
fn reduced_faction_reputations_keep_selected_rules_and_physical_targets() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = archive(scratch.path())?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    let selection = ReputationSelection {
        faction_reputations: Some(CatalogRecordSelection::All),
        contexts: Some(CatalogRecordSelection::All),
        perks: Some(CatalogRecordSelection::All),
        ..ReputationSelection::ranks()
    };
    session.with_reputation(selection, None, |snapshot| {
        validate(&snapshot, scratch.path())
    })??;
    Ok(())
}

fn validate(snapshot: &ReputationSnapshot<'_>, scratch: &Path) -> TestResult {
    let rows = snapshot
        .faction_reputations
        .as_ref()
        .ok_or("faction rows")?;
    let scopes = targets(snapshot.scopes.as_ref(), |scope| scope.source())?;
    let standings = targets(snapshot.standings.as_ref(), |standing| standing.source())?;
    let contexts = targets(snapshot.contexts.as_ref(), |context| context.source())?;
    let perks = targets(snapshot.perks.as_ref(), |perk| perk.source())?;
    let dependencies = Dependencies {
        scopes: &scopes,
        standings: &standings,
        contexts: &contexts,
        perks: &perks,
    };
    let mut references = TextRefs::default();
    let staged = faction_reputations::stage(rows, &dependencies, scratch, None, &mut references)?;
    verify_targets(rows.rows.iter().map(|row| row.source()), &staged.targets)?;
    let table = serde_json::to_value(staged.table)?;
    let original = table.pointer("/rows/0").ok_or("primary faction row")?;
    verify_links(original)?;
    let mut texts = SelectedTexts::default();
    texts.add(
        references,
        &snapshot.public_names,
        &snapshot.game_texts,
        None,
    )?;
    verify_texts(original, &serde_json::to_value(texts)?)?;

    let missing_standing = Dependencies {
        standings: &RecordTargets::default(),
        ..dependencies
    };
    assert!(
        matches!(
            faction_reputations::stage(
                rows,
                &missing_standing,
                scratch,
                None,
                &mut TextRefs::default()
            ),
            Err(ReducedError::UnresolvedRecordLink {
                path: PATH,
                index: 1
            })
        ),
        "a resolved standing must exist in the independent, staged standings table"
    );
    Ok(())
}

fn verify_targets<'read>(
    headers: impl Iterator<Item = &'read CanonicalRecordHeader<'read>>,
    targets: &RecordTargets,
) -> TestResult {
    for header in headers {
        targets.verify(
            PATH,
            *header
                .record_index()
                .value()
                .ok_or("physical record index")?,
            *header.guid().value().ok_or("physical record GUID")?,
        )?;
    }
    Ok(())
}

fn targets<'read, T: 'read>(
    source: Option<&'read ReputationRows<T>>,
    header: impl Fn(&'read T) -> &'read CanonicalRecordHeader<'read>,
) -> TestResult<RecordTargets> {
    let mut targets = RecordTargets::default();
    for original in &source.ok_or("selected target family")?.rows {
        targets.insert(PATH, header(original))?;
    }
    Ok(targets)
}

fn verify_links(row: &Value) -> TestResult {
    assert_eq!(
        row.pointer("/recordIndex/value"),
        Some(&json!(5)),
        "physical faction identity"
    );
    assert_eq!(
        row.pointer("/name/value"),
        Some(&json!("fixture_reputation_owner")),
        "authored technical name"
    );
    assert_eq!(
        row.pointer("/isNpc/value"),
        Some(&json!(true)),
        "authored NPC flag"
    );
    assert_eq!(
        row.pointer("/isNpc/containsEncryptedSource"),
        Some(&json!(true)),
        "actual source marker"
    );

    let rule = "/hostilityParams/value";
    for (location, index) in [
        ("/allies/value/0".to_owned(), 6),
        ("/enemies/value/1".to_owned(), 6),
        ("/reputationContextPropertiesUi".to_owned(), 2),
        ("/perkReputationRewardList/value/0".to_owned(), 3),
        (format!("{rule}/scope"), 0),
        (format!("{rule}/standing"), 1),
    ] {
        assert_eq!(
            row.pointer(&format!("{location}/value/targetRecordIndex/value")),
            Some(&json!(index)),
            "exact physical target at {location}"
        );
    }
    assert_eq!(
        row.pointer("/enemies/value/0/value/status"),
        Some(&json!("dangling")),
        "dangling enemy slot is preserved"
    );
    assert!(
        row.get("authoredFields").is_none(),
        "no raw source arena for the row"
    );
    assert!(
        row.get("description").is_none(),
        "only the selected rule description is exported"
    );
    assert_eq!(
        row.pointer("/hostilityParams/value/markerParams/value/icon/value"),
        Some(&json!("textures/fixture/relation.dds")),
        "the typed relation marker retains its authored icon"
    );
    Ok(())
}

fn verify_texts(row: &Value, texts: &Value) -> TestResult {
    let label = row
        .pointer("/displayName/value")
        .ok_or("protected faction label")?;
    let marker = row
        .pointer("/hostilityParams/value/markerParams/value/description/value")
        .ok_or("typed marker description")?;
    assert_eq!(
        marker.get("type"),
        Some(&json!("localized")),
        "preserve the actual Locale variant"
    );
    let marker_key = marker
        .get("reference")
        .ok_or("protected marker reference")?;
    let selected = texts
        .get("gameTexts")
        .and_then(Value::as_array)
        .ok_or("protected text rows")?;
    assert_eq!(selected.len(), 2, "only keys referenced by retained cells");
    for (key, english, german) in [
        (label, "Faction", "Fraktion"),
        (marker_key, "Relation", "Beziehung"),
    ] {
        let text = selected
            .iter()
            .find(|entry| entry.pointer("/id/value") == Some(key))
            .ok_or("exact selected key")?;
        assert_eq!(
            text.pointer("/translations/value/en/displayText/value"),
            Some(&json!(english)),
            "English selected text"
        );
        assert_eq!(
            text.pointer("/translations/value/de/displayText/value"),
            Some(&json!(german)),
            "German selected text"
        );
    }
    assert_eq!(
        texts.get("publicNames"),
        Some(&json!([])),
        "protected keys stay in their canonical domain"
    );
    Ok(())
}

fn archive(folder: &Path) -> TestResult<Archive> {
    let path = folder.join("Data.p4k");
    let ini = |faction, marker| {
        GlobalIniBuilder::new()
            .text("shared", faction)
            .text("own_description", marker)
            .text("unused_fixture", "Unused")
            .build()
    };
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", fixture::owner(false).build()).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            ini("Faction", "Relation"),
        ))
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            ini("Fraktion", "Beziehung"),
        ))
        .write_to(&path)?;
    Ok(Archive::open_with_key(path, Some(test_key()))?)
}
