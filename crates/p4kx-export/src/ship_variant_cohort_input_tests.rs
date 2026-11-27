// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Filter, Record};

use super::super::variant_operands::probe::{self, DefinitionMembership};
use super::fixture::{self, Fault, Origin, TestResult};
use crate::catalog_projection::CatalogProjection;
use crate::entity_selection::{self, Selected};
use crate::guid_text::CatalogGuid;
use crate::playability_rules::Evidence;
use crate::record_fields;
use crate::record_observation::RecordObservation;
use crate::variants::{ShipDefinitionKey, VariantBases, VariantMember, VariantRank};
use crate::{Kind, Playability};

#[test]
fn source_only_inputs_keep_complete_absence_and_unreadable_prefix_distinct() -> TestResult {
    for (fault, expected) in [
        (
            Fault::None,
            DefinitionMembership::Comparable(ShipDefinitionKey::new(
                "  data/vehicles/original.xml  ",
            )),
        ),
        (Fault::EarlierComponent, DefinitionMembership::Unavailable),
    ] {
        fixture::with_projection(
            &fixture::database(fault),
            Origin::Encrypted,
            |forge, projection| {
                for (index, membership) in [(0, &expected), (1, &DefinitionMembership::Excluded)] {
                    let record = forge.record(index).ok_or("one original Ship owner")?;
                    let tree = record.tree()?;
                    let input = projection.original();
                    let admission = RecordObservation::capture(input, &record)?;
                    let original = probe::read(&tree, input, &admission, None)?;
                    let canonical = super::super::read(&tree, projection, &admission, None)?;
                    assert_eq!(
                        original.definition, canonical.vehicle_definition,
                        "the streaming membership read uses the canonical original source cell"
                    );
                    assert_eq!(
                        original.definition.origin().contains_encrypted_source(),
                        Some(true),
                        "earlier uncertainty does not erase the encrypted original source"
                    );
                    assert_eq!(
                        &DefinitionMembership::of(&original.definition),
                        membership,
                        "only complete selected fields can prove which cohort to inspect"
                    );
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn exact_rank_inputs_reach_the_player_outside_output_without_its_same_guid_alias() -> TestResult {
    fixture::with_projection(
        &fixture::variant_candidates(),
        Origin::Encrypted,
        |forge, projection| {
            let ship = forge.record(0).ok_or("the selected physical Ship")?;
            let path = ship.record_path()?;
            let filter = Filter::new(&format!("?{}", path.get(1..).ok_or("fixture path")?));
            let expected = ShipDefinitionKey::new("data/vehicles/match.xml");
            let mut ranks = VariantBases::new();
            let mut observed = 0;
            for record in forge.records() {
                assert_eq!(
                    entity_selection::select(&record, None)?,
                    Some(Selected::Entity(Kind::Ships)),
                    "the scope enumerates canonical unfiltered Ship owners"
                );
                assert_eq!(
                    filter.matches(record.record_path()?),
                    record.index() == ship.index(),
                    "the producer output filter excludes the winning original and its alias"
                );
                if let Some(rank) = candidate_rank(&record, projection, &expected)? {
                    ranks.observe(&expected, rank);
                    observed += 1;
                }
            }
            assert_eq!(
                observed, 2,
                "the unrelated physical alias contributes no rank"
            );
            let winner = ranks
                .base_for(&&expected, CatalogGuid(ship.guid()))
                .ok_or("matching original rank")?;
            assert_eq!(
                winner.record_index,
                fixture::VARIANT_BASE_INDEX,
                "the prospective cohort chooses the exact original, not a GUID-only alias"
            );
            Ok(())
        },
    )
}

fn candidate_rank<'source>(
    record: &Record<'source>,
    projection: &CatalogProjection<'_, 'source>,
    expected: &ShipDefinitionKey,
) -> Result<Option<VariantRank<'source>>, Box<dyn Error>> {
    let tree = record.tree()?;
    let input = projection.original();
    let admission = RecordObservation::capture(input, record)?;
    let original = probe::read(&tree, input, &admission, None)?;
    if !matches!(
        DefinitionMembership::of(&original.definition),
        DefinitionMembership::Comparable(actual) if &actual == expected
    ) {
        return Ok(None);
    }
    let canonical = super::super::read(&tree, projection, &admission, None)?;
    let (evidence, _) = Evidence::read_for_ships([&tree], input, None)?;
    let rank = original
        .playability
        .decide(&tree, original.vehicle, &evidence, input)?;
    let wanted = canonical
        .playability_proof
        .decide(&tree, canonical.vehicle, &evidence, input)?;
    assert_eq!(
        rank, wanted,
        "the original rank decision and source cells match"
    );
    assert_eq!(
        rank.value() == Some(&Playability::Player),
        record.index() == fixture::VARIANT_BASE_INDEX,
        "the physically reached player outranks the lexical first selected Ship"
    );
    Ok(Some(VariantRank {
        member: VariantMember {
            guid: CatalogGuid(record.guid()),
            record_index: record.index(),
        },
        class_name: record_fields::class_name(&tree),
        playability: rank.value().copied(),
    }))
}
