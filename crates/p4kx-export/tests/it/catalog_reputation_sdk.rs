// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

#[path = "catalog_reputation_sdk_views.rs"]
mod views;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ReputationSelection,
    ReputationSnapshot, ResourceNumber, with_reputation_from,
};
use p4kx_export::{CatalogError, CatalogOptions, build_catalog};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde::Serialize;
use serde_json::Value;

use super::{TestResult, guid};
use crate::reputation_fixture::{FACTION, STANDING_HIGH, STANDING_LOW, write_archive};

#[test]
fn reputation_queries_preserve_canonical_rows_and_scope_links() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let built = build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let high_index = parsed.with_reputation(ReputationSelection::all(), None, |snapshot| {
        let factions = snapshot
            .faction_reputations
            .as_ref()
            .ok_or("requested faction reputations")?;
        let scopes = snapshot.scopes.as_ref().ok_or("requested scopes")?;
        let standings = snapshot.standings.as_ref().ok_or("requested standings")?;
        compare_rows(&factions.rows, &catalog, "factionReputations")?;
        compare_rows(&scopes.rows, &catalog, "reputationScopes")?;
        compare_rows(&standings.rows, &catalog, "reputationStandings")?;
        check_faction_name(&snapshot)?;
        views::check_scope_progression(&snapshot)?;
        check_scope_links(&snapshot)
    })??;
    let selections = [
        CatalogRecordSelection::Guid(guid(STANDING_HIGH).parse()?),
        CatalogRecordSelection::RecordIndex(high_index),
    ];
    for selection in selections {
        let mut request = ReputationSelection::default();
        request.standings = Some(selection);
        parsed.with_reputation(request, None, |snapshot| {
            assert!(snapshot.faction_reputations.is_none(), "the query only selects standings");
            assert!(snapshot.scopes.is_none(), "the query leaves scopes out of the result");
            let standings = snapshot.standings.ok_or("selected standing table")?;
            let [row] = standings.rows.as_slice() else {
                return Err("exactly one selected standing".into());
            };
            assert_eq!(row.source().record_index().value(), Some(&high_index), "both selectors identify the same original standing");
            assert!(matches!(row.min_reputation().value(), Some(ResourceNumber::Int64 { value }) if value.get() == 4500), "the exact signed threshold is preserved");
            Ok::<(), Box<dyn Error>>(())
        })??;
    }
    Ok(())
}

fn compare_rows(rows: &[impl Serialize], catalog: &Value, dataset: &str) -> TestResult {
    let expected = catalog
        .pointer(&format!("/definitions/{dataset}/rows"))
        .ok_or("definition table")?;
    assert_eq!(
        &serde_json::to_value(rows)?,
        expected,
        "canonical {dataset} rows"
    );
    Ok(())
}

fn check_faction_name(snapshot: &ReputationSnapshot<'_>) -> TestResult {
    let factions = snapshot
        .faction_reputations
        .as_ref()
        .ok_or("faction reputations")?;
    let faction_guid = guid(FACTION).parse()?;
    let faction = factions
        .rows
        .iter()
        .find(|row| row.source().guid().value() == Some(&faction_guid))
        .ok_or("faction reputation row")?;
    let reference = faction
        .display_name()
        .value()
        .ok_or("original label reference")?;
    let text = snapshot
        .game_text(reference)
        .ok_or("selected protected text")?;
    assert_eq!(
        text.key().value(),
        Some(&"@faction_acme"),
        "the key comes from the original faction reputation"
    );
    let translation = text
        .translations()
        .value()
        .and_then(|rows| rows.get("en"))
        .and_then(|row| row.display_text.value())
        .ok_or("English display text")?;
    assert_eq!(
        translation.as_ref(),
        "Acme Emporium",
        "the scoped language resolves the expected display text"
    );
    Ok(())
}

fn check_scope_links(snapshot: &ReputationSnapshot<'_>) -> Result<u32, Box<dyn Error>> {
    let scopes = snapshot.scopes.as_ref().ok_or("requested scope table")?;
    let [scope] = scopes.rows.as_slice() else {
        return Err("one authored scope".into());
    };
    let slots = scope
        .standings()
        .and_then(|rows| rows.value())
        .ok_or("standing map")?;
    let [low, high] = slots.as_slice() else {
        return Err("two authored standing references".into());
    };
    for (slot, expected) in [(low, STANDING_LOW), (high, STANDING_HIGH)] {
        let link = slot.value().ok_or("original link")?;
        let joined = snapshot.standing(link).ok_or("physical standing target")?;
        assert_eq!(
            joined.source().guid().value(),
            Some(&guid(expected).parse()?),
            "the slot resolves the original standing GUID"
        );
        assert_eq!(
            joined.source().record_index().value(),
            link.target_record_index().value(),
            "the join uses the physical DataForge Record"
        );
        for reference in slot.origin().source_uses_refs() {
            assert!(
                snapshot
                    .source_inventory
                    .iter_uses()
                    .any(|item| item.id() == reference),
                "the slot origin resolves within the snapshot inventory"
            );
        }
    }
    Ok(high
        .value()
        .and_then(|link| link.resolved_index())
        .ok_or("physical high standing target")?)
}

#[test]
fn out_of_family_reputation_lookup_does_not_open_a_locked_locale() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", rank_database()))
        .entry(
            TestEntry::stored(
                Language::English.global_ini_path(),
                b"rank_key=Synthetic rank\n",
            )
            .encrypted(),
        )
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let language = CatalogLanguageSelection::Selected(&[Language::English]);
    let mut request = ReputationSelection::default();
    request.standings = Some(CatalogRecordSelection::RecordIndex(1));
    with_reputation_from(
        &archive,
        &entry,
        request,
        language,
        None,
        check_empty_standing_selection,
    )?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        0,
        "the encrypted locale was never attempted"
    );
    let cancel = CancelToken::default();
    cancel.cancel();
    request.standings = Some(CatalogRecordSelection::RecordIndex(0));
    assert!(
        matches!(
            with_reputation_from(
                &archive,
                &entry,
                request,
                language,
                Some(&cancel),
                |_| panic!("cancelled read")
            ),
            Err(CatalogError::Cancelled)
        ),
        "the cancelled request stops before the selected standing is read"
    );
    assert!(
        matches!(
            with_reputation_from(&archive, &entry, request, language, None, |_| ()),
            Err(CatalogError::LoadTexts(_))
        ),
        "a selected standing requires its requested locale payload"
    );
    Ok(())
}

fn check_empty_standing_selection(snapshot: ReputationSnapshot<'_>) {
    assert!(
        snapshot.faction_reputations.is_none(),
        "the query only selects standings"
    );
    assert!(
        snapshot
            .standings
            .is_some_and(|result| result.rows.is_empty()),
        "the selected out-of-family Record does not make a standing row"
    );
    assert_eq!(
        snapshot.requested_languages,
        [Language::English],
        "retain the request locale even for an empty selection"
    );
    assert!(
        snapshot.available_languages.is_empty(),
        "empty selections need no locale payloads"
    );
}

fn rank_database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("SReputationStandingParams").field("displayName", TestType::Locale),
        )
        .structure(TestStruct::new("Unrelated"))
        .record(
            TestRecord::new(
                1,
                "SReputationStandingParams",
                "SReputationStandingParams.Fixture",
                "libs/foundry/records/reputation/standings/faction/fixture.xml",
            )
            .value("displayName", TestValue::text("@rank_key")),
        )
        .record(TestRecord::new(
            2,
            "Unrelated",
            "Unrelated.Other",
            "libs/foundry/records/unrelated.xml",
        ))
        .build()
}
