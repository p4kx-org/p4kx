// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    ReputationSnapshot, ResourceNumber, ScopeStandingMap, StandingDefinition, StandingPresentation,
    StandingProgression,
};

use p4kx_export::mining_locations::ResourceFloat32;

use super::{check_mixed_translation, check_origin};
use crate::commodity_v3_fixture::guid;
use crate::reputation_fixture::{STANDING_HIGH, STANDING_LOW};

pub(crate) fn check_scope_progression(snapshot: &ReputationSnapshot<'_>) -> super::TestResult {
    let scopes = snapshot.scopes.as_ref().ok_or("scope table")?;
    let [scope] = scopes.rows.as_slice() else {
        return Err("one original scope".into());
    };
    let map: &ScopeStandingMap<'_> = scope
        .standing_map()
        .value()
        .ok_or("original standing map")?;
    assert!(
        matches!(map.reputation_ceiling().value(), Some(ResourceNumber::Int64 { value }) if value.get() == 9_007_199_254_740_993),
        "the ceiling stays exact beyond binary64 integer precision"
    );
    assert!(
        matches!(map.initial_reputation().value(), Some(ResourceNumber::Int64 { value }) if value.get() == 0),
        "zero is a present original value"
    );
    check_origin(snapshot, map.reputation_ceiling(), Some(true))?;
    for root in map.authored_fields().roots() {
        assert!(
            map.authored_fields().value(root.value()).is_some(),
            "standing-map extras keep owner-local references"
        );
    }
    let standings = snapshot.standings.as_ref().ok_or("standing table")?;
    for (id, bits, gated) in [
        (STANDING_LOW, (-0.0_f32).to_bits(), false),
        (STANDING_HIGH, 24.5_f32.to_bits(), true),
    ] {
        let wanted = guid(id).parse()?;
        let standing = standings
            .rows
            .iter()
            .find(|row| row.source().guid().value() == Some(&wanted))
            .ok_or("original standing")?;
        check_standing(snapshot, standing, bits, gated)?;
    }
    Ok(())
}

fn check_standing(
    snapshot: &ReputationSnapshot<'_>,
    standing: &StandingDefinition<'_>,
    bits: u32,
    gated: bool,
) -> super::TestResult {
    let progression: &StandingProgression = standing.progression();
    let presentation: &StandingPresentation<'_> = standing.presentation();
    assert_eq!(
        progression
            .drift_time_hours()
            .value()
            .map(ResourceFloat32::raw_bits),
        Some(bits),
        "the original source Single remains hours, including signed zero"
    );
    assert_eq!(
        progression.gated().value(),
        Some(&gated),
        "the original gate flag keeps false and true distinct"
    );
    assert!(
        matches!(progression.drift_reputation().value(), Some(ResourceNumber::Int64 { value }) if value.get() == -5),
        "negative drift remains the original signed integer"
    );
    check_origin(snapshot, progression.drift_time_hours(), Some(true))?;
    let reference = presentation
        .perk_description()
        .value()
        .ok_or("protected perk reference")?;
    let text = snapshot
        .game_text(reference)
        .ok_or("same-domain perk text")?;
    let translated = text
        .translations()
        .value()
        .and_then(|rows| rows.get("en"))
        .ok_or("original English perk text")?;
    check_origin(snapshot, &translated.display_text, Some(true))?;
    check_mixed_translation(snapshot, &translated.display_text)?;
    for field in standing.additional_fields().roots() {
        assert!(
            standing.additional_fields().value(field.value()).is_some(),
            "standing extras remain traversable in their original arena"
        );
    }
    Ok(())
}
