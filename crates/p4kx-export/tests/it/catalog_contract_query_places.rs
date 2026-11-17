// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, ContractSnapshot, with_contracts,
};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use serde_json::Value;

use super::TestResult;
use crate::commodity_v3_fixture::{self, DcbOrigin, guid};
use crate::health_origin_assertions as proof;

#[path = "catalog_contract_query_place_fixture.rs"]
mod fixture;

#[test]
fn contract_sdk_starmap_join_rejects_a_source_outside_pu() -> TestResult {
    verify(fixture::Case::OutsideOnly, false)
}

#[test]
fn contract_sdk_starmap_join_preserves_the_selected_source_against_later_aliases() -> TestResult {
    for case in [fixture::Case::OutsideShadow, fixture::Case::LeftoverShadow] {
        verify(case, true)?;
    }
    Ok(())
}

fn verify(case: fixture::Case, has_place: bool) -> TestResult {
    let (_folder, archive) =
        commodity_v3_fixture::open_bytes(fixture::database(case), DcbOrigin::Plain)?;
    let options = p4kx_export::CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..commodity_v3_fixture::options()
    };
    let built = build_catalog(&archive, &options)?;
    let canonical: Value = serde_json::from_slice(built.json())?;
    let [expected] = proof::rows(&canonical, "/projections/contracts/rows")? else {
        return Err("one canonical contract row".into());
    };
    assert_eq!(
        expected["pickupPlaces"].as_array().map(Vec::len),
        Some(usize::from(has_place)),
        "fixture {case:?} exercises canonical place admission"
    );
    for selection in [
        CatalogRecordSelection::All,
        CatalogRecordSelection::Guid(guid(fixture::OWNER).parse()?),
    ] {
        with_contracts(
            &archive,
            selection,
            CatalogLanguageSelection::Selected(&[]),
            None,
            |snapshot| verify_snapshot(&snapshot, expected, has_place),
        )??;
    }
    Ok(())
}

fn verify_snapshot(
    snapshot: &ContractSnapshot<'_>,
    expected: &Value,
    has_place: bool,
) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one selected canonical contract".into());
    };
    assert_eq!(
        serde_json::to_value(&row.base.pickup_places)?,
        expected["pickupPlaces"],
        "pickup places use the complete producer's path, type and leftover admission"
    );
    assert_eq!(
        serde_json::to_value(&row.base.offered_at)?,
        expected["offeredAt"],
        "authored location identity survives without adopting an excluded source's name"
    );
    assert!(
        snapshot.coverage.projection_failures.is_empty(),
        "exclusion is a complete source policy decision"
    );
    assert!(
        snapshot.coverage.dependencies.forward_failures.is_empty(),
        "excluded readable sources are not read failures"
    );
    assert!(
        snapshot
            .coverage
            .dependencies
            .forward_records
            .windows(2)
            .all(|pair| matches!(pair, [first, second] if first <= second)),
        "forward candidate coverage uses physical source order"
    );
    let mut keys = snapshot
        .game_texts
        .iter()
        .filter_map(|text| text.key().value().copied());
    assert!(
        !keys.any(|key| key == fixture::REJECTED_KEY),
        "an outside or leftover source does not register a text payload"
    );
    if has_place {
        let [place] = row.base.pickup_places.as_slice() else {
            return Err("one admitted place".into());
        };
        let name = place
            .place
            .as_ref()
            .and_then(|cell| cell.value())
            .ok_or("the admitted name cell")?;
        let text = snapshot
            .game_texts
            .iter()
            .find(|text| text.id().value() == Some(name))
            .ok_or("the admitted place's canonical text row")?;
        assert_eq!(
            text.key().value().copied(),
            Some(fixture::ACCEPTED_KEY),
            "a later excluded alias cannot override the admitted physical name source"
        );
    }
    Ok(())
}
