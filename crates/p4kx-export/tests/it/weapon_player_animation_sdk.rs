// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::equipment::{
    WeaponAnimationPlayback, WeaponAnimationTagEvidence, WeaponPlayerAnimationCandidate,
    with_weapon_animation_candidates_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, DatasetAvailability,
};
use p4kx_extraction::SourceValueState;
use serde_json::json;

use super::fixture::{OTHER_ITEM, WEAPON, database, database_with_player_assets, documents};
use super::{Fixture, Input, TestResult, origin};

const ROWS: &str = "/projections/weaponPlayerAnimationCandidates/rows";

#[test]
fn weapon_animation_sdk_matches_scoped_canonical_candidates_and_origins() -> TestResult {
    let data = database();
    for input in [Input::Plain, Input::Encrypted] {
        let fixture = Fixture::open(&data, input, documents())?;
        let (_, catalog) = fixture.diagnostic()?;
        full_proof::from_entry(
            &fixture.archive,
            &fixture.entry,
            full_proof::NO_LANGUAGES,
            None,
        )?
        .with_weapon_animation_candidates(WEAPON, None, |result| -> TestResult {
            assert!(matches!(
                result.availability,
                DatasetAvailability::Available
            ));
            assert!(
                result.coverage.unreadable_bodies.is_empty(),
                "the selected Item is readable"
            );
            assert!(
                result.coverage.incomplete_fields.is_empty(),
                "the selected Item's fields are readable"
            );
            assert_eq!(
                result.graph_coverage.player_state,
                Some(SourceValueState::Present)
            );
            assert_eq!(result.graph_coverage.player_candidates, 1);
            assert_eq!(result.graph_coverage.unresolved_branches, 1);
            assert_eq!(result.graph_coverage.cyclic_branches, 1);
            assert!(
                result.dependency_coverage.failed_entries().is_empty(),
                "all reached physical source Entries were readable"
            );
            assert_eq!(
                result.rows.len(),
                2,
                "two physical options in the matched fragment scope"
            );
            for row in &result.rows {
                check_candidate(row, input);
            }
            let scoped = json!({
                "projections":{"weaponPlayerAnimationCandidates":{"rows":result.rows}},
                "provenance": result.source_inventory,
            });
            assert_eq!(
                origin::rows(&scoped, ROWS)?,
                origin::rows(&catalog, ROWS)?,
                "same rows, order and exact contributors as the producer"
            );
            origin::closure(&scoped)
        })??;
    }
    Ok(())
}

fn check_candidate(row: &WeaponPlayerAnimationCandidate, input: Input) {
    assert_eq!(
        row.weapon_record_index().value(),
        Some(&WEAPON),
        "the candidate retains its weapon owner"
    );
    assert_eq!(
        row.attachment_component_ordinal().value(),
        Some(&0),
        "the original attachment slot stays addressable"
    );
    assert!(
        matches!(
            row.tag_evidence().value(),
            Some(WeaponAnimationTagEvidence::SharedDeclaration)
        ),
        "both token positions resolve to the same declaration"
    );
    assert!(
        matches!(row.playback(), WeaponAnimationPlayback::Unverified),
        "a source candidate cannot establish runtime playback"
    );
    assert_eq!(
        row.tag_evidence().origin().contains_encrypted_source(),
        input.marker(),
        "the candidate inherits its actual DataForge origin"
    );
    assert_eq!(
        row.matches().len(),
        1,
        "the original option and weapon tokens share one declaration"
    );
    assert_ne!(
        row.branch_database(),
        row.option_database(),
        "original parent and option entries remain distinct"
    );
}

#[test]
fn weapon_animation_sdk_leaves_the_player_graph_outside_a_non_weapon_scope() -> TestResult {
    let fixture = Fixture::open(&database(), Input::Encrypted, documents())?;
    let session = CatalogQuerySession::from_entry(
        &fixture.archive,
        &fixture.entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_weapon_animation_candidates(OTHER_ITEM, None, |result| {
        assert!(matches!(result.availability, DatasetAvailability::Absent));
        assert!(
            result.rows.is_empty(),
            "no personal-weapon source was admitted for the selected Item"
        );
        assert!(
            result.graph_coverage.player_state.is_none(),
            "the Player graph was not needed"
        );
        assert!(
            result.coverage.unreadable_bodies.is_empty(),
            "known absence follows a readable owner"
        );
        assert!(
            result.dependency_coverage.failed_entries().is_empty(),
            "no animation asset reads were required"
        );
    })?;
    Ok(())
}

#[test]
fn weapon_animation_sdk_reports_wrong_kind_player_assets() -> TestResult {
    let data =
        database_with_player_assets("Animations/TestRoot.adb", "Animations/TestController.xml");
    let fixture = Fixture::open(&data, Input::Plain, documents())?;
    with_weapon_animation_candidates_from(
        &fixture.archive,
        &fixture.entry,
        WEAPON,
        None,
        |result| {
            assert_eq!(result.graph_coverage.unresolved_controllers, 1);
            assert_eq!(result.graph_coverage.unresolved_databases, 1);
            assert!(
                result.rows.is_empty(),
                "neither authored link identifies its expected target kind"
            );
        },
    )?;
    Ok(())
}
