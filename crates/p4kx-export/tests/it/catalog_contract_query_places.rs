// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{CatalogRecordSelection, ContractSnapshot};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, StructValue, Value as Raw};
use serde_json::{Value, json};

use super::TestResult;
use crate::commodity_v3_fixture::{self, DcbOrigin, guid};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

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

#[test]
fn contract_projections_follow_the_physically_resolved_locality() -> TestResult {
    for (case, has_place) in [
        (fixture::LocalityCase::SameFamily, true),
        (fixture::LocalityCase::ForeignFirst, false),
    ] {
        let bytes = fixture::locality_database(case);
        let original = original_locality_record(&bytes)?;
        assert_eq!(
            original, 1,
            "the stored Reference resolves to the first physical owner"
        );
        let (_folder, archive) = commodity_v3_fixture::open_bytes(bytes, DcbOrigin::Plain)?;
        let options = p4kx_export::CatalogOptions {
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..commodity_v3_fixture::options()
        };
        let catalog: Value = serde_json::from_slice(
            build_catalog(&archive, options.with_full_provenance())?.json(),
        )?;
        let [contract] = proof::rows(&catalog, "/projections/contracts/rows")? else {
            return Err("one original contract".into());
        };
        assert_eq!(
            contract.pointer("/offeredAt/localityGuids"),
            Some(&json!([guid(fixture::LOCALITY)])),
            "the published Locality GUID remains the original nominal identity"
        );
        let locations = proof::rows(contract, "/pickupPlaces")?;
        let expected = if has_place {
            vec![guid(fixture::LOCATION)]
        } else {
            Vec::new()
        };
        let actual = locations
            .iter()
            .map(|place| place["starmapObjectGuid"].as_str().ok_or("pickup GUID"))
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            actual, expected,
            "the other physical Locality cannot supply a pickup place"
        );
        full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_contracts(
            CatalogRecordSelection::RecordIndex(0),
            None,
            |snapshot| {
                verify_snapshot(&snapshot, contract, has_place)?;
                let exact = [original, 3];
                let expected: &[u32] = if has_place { &exact } else { &[] };
                assert_eq!(
                    snapshot.coverage.dependencies.forward_records.as_slice(),
                    expected,
                    "the selected Locality and its location are the only admitted dependencies"
                );
                Ok::<(), Box<dyn std::error::Error>>(())
            },
        )??;
    }
    Ok(())
}

fn original_locality_record(bytes: &[u8]) -> TestResult<u32> {
    let forge = DataForge::open(bytes)?;
    let source = forge
        .record(0)
        .ok_or("the original generator Record")?
        .tree()?;
    let handler = first_member(source.root(), "generators")?;
    let contract = first_member(handler, "contracts")?;
    let prerequisite = first_member(contract, "additionalPrerequisites")?;
    let Some(Raw::Reference(reference)) = prerequisite.field("localityAvailable") else {
        return Err("an original Record Reference for the locality".into());
    };
    assert_eq!(
        reference.guid().to_string(),
        guid(fixture::LOCALITY),
        "the authored Reference holds the aliased GUID"
    );
    Ok(reference.record_index())
}

fn first_member<'read, 'source>(
    parent: &'read StructValue<'source>,
    field: &str,
) -> TestResult<&'read StructValue<'source>> {
    let Some(Raw::Array(members)) = parent.field(field) else {
        return Err(format!("missing original {field} array").into());
    };
    let [Raw::Struct(member)] = members.as_ref() else {
        return Err(format!("one original owned {field} member").into());
    };
    Ok(member)
}

fn verify(case: fixture::Case, has_place: bool) -> TestResult {
    let (_folder, archive) =
        commodity_v3_fixture::open_bytes(fixture::database(case), DcbOrigin::Plain)?;
    let options = p4kx_export::CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..commodity_v3_fixture::options()
    };
    let built = build_catalog(&archive, options.with_full_provenance())?;
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
        full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_contracts(
            selection,
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
