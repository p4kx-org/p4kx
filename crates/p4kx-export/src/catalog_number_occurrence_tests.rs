// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use crate::catalog_number_capture::{CapturedNumber, FloatToken, NumberOccurrence};
use crate::catalog_row::CatalogRows;
use crate::catalog_target_index::TargetIndex;
use crate::resource_raw_policy::ResourceNumberContract;
use crate::selection::Kind;

use crate::catalog_number_health::{EmittedNumber, NumberCaptures, NumberRequirement};

fn captured(ordinal: u64) -> CapturedNumber {
    CapturedNumber {
        record_index: 7,
        occurrence: NumberOccurrence::LootValue { ordinal },
        field: "filter.same".to_owned(),
        token: FloatToken::Single(0x7fc0_0017),
        source_type: "PoolFilter_Custom".to_owned(),
        contract: ResourceNumberContract::Number,
    }
}

fn requirement(ordinal: u64) -> NumberRequirement {
    NumberRequirement {
        record_index: 7,
        original_ordinal: ordinal,
        field: "filter.same".to_owned(),
        source_type: "PoolFilter_Custom".to_owned(),
        contract: None,
        token: FloatToken::Single(0x7fc0_0017),
        body: None,
    }
}

fn rows(kind: Kind, values: Vec<CapturedNumber>) -> CatalogRows {
    CatalogRows {
        numbers: values
            .into_iter()
            .map(|value| EmittedNumber { kind, value })
            .collect(),
        ..CatalogRows::default()
    }
}

fn targets(kind: Kind, physical: u32) -> TargetIndex {
    TargetIndex {
        emitted: HashSet::from([(kind, physical)]),
        emitted_by_guid: HashMap::new(),
        ambiguous: HashSet::new(),
        ambiguous_sources: Vec::new(),
        excluded: HashMap::new(),
    }
}

#[test]
fn original_loot_occurrences_cover_exactly_once_despite_equal_names_and_bits() {
    let rows = rows(Kind::LootPoolFilters, vec![captured(3), captured(4)]);
    let targets = targets(Kind::LootPoolFilters, 7);
    let indexed = NumberCaptures::new(Some(&rows), Some(&targets));
    let mut coverage = indexed.for_record(7, Some(Kind::LootPoolFilters));
    assert!(
        coverage.take(&requirement(3)),
        "first genuine original occurrence is covered"
    );
    assert!(
        coverage.take(&requirement(4)),
        "same-name second original occurrence needs its own receipt"
    );
    assert!(
        !coverage.take(&requirement(3)),
        "one actual emitted receipt cannot cover two original requirements"
    );
}

#[test]
fn both_actual_emitted_family_and_current_source_family_are_required_for_loot() {
    for kind in [
        Kind::Commodities,
        Kind::ResourceTypes,
        Kind::GameplayProperties,
        Kind::QualityDistributions,
    ] {
        let rows = rows(kind, vec![captured(3)]);
        let targets = targets(kind, 7);
        let indexed = NumberCaptures::new(Some(&rows), Some(&targets));
        assert!(
            !indexed
                .for_record(7, Some(Kind::LootPoolFilters))
                .take(&requirement(3)),
            "a foreign emitted family cannot supply Loot occurrence evidence"
        );
    }
    let rows = rows(Kind::LootPoolFilters, vec![captured(3)]);
    let targets = targets(Kind::LootPoolFilters, 7);
    let indexed = NumberCaptures::new(Some(&rows), Some(&targets));
    for kind in [
        None,
        Some(Kind::Commodities),
        Some(Kind::GameplayProperties),
        Some(Kind::QualityQuantizations),
    ] {
        assert!(
            !indexed.for_record(7, kind).take(&requirement(3)),
            "a Loot receipt cannot waive another actual source family"
        );
    }
}

#[test]
fn missing_emission_and_wrong_physical_owner_never_cover_a_token() {
    let rows = rows(Kind::LootPoolFilters, vec![captured(3)]);
    let wrong_targets = targets(Kind::LootPoolFilters, 8);
    let indexed = NumberCaptures::new(Some(&rows), Some(&wrong_targets));
    assert!(
        !indexed
            .for_record(7, Some(Kind::LootPoolFilters))
            .take(&requirement(3)),
        "nominal row data does not prove actual physical emission"
    );
    let targets = targets(Kind::LootPoolFilters, 7);
    let indexed = NumberCaptures::new(Some(&rows), Some(&targets));
    assert!(
        !indexed
            .for_record(8, Some(Kind::LootPoolFilters))
            .take(&requirement(3)),
        "the current observation belongs to another physical Record"
    );
    assert!(
        !NumberCaptures::new(None, Some(&targets))
            .for_record(7, Some(Kind::LootPoolFilters))
            .take(&requirement(3)),
        "no actual row snapshot means no receipt"
    );
}

#[test]
fn ordinal_width_bits_type_path_and_original_cardinality_remain_load_bearing() {
    let rows = rows(Kind::LootPoolFilters, vec![captured(3)]);
    let targets = targets(Kind::LootPoolFilters, 7);
    let indexed = NumberCaptures::new(Some(&rows), Some(&targets));
    let mut wrong = requirement(3);
    wrong.original_ordinal = 4;
    assert!(
        !indexed
            .for_record(7, Some(Kind::LootPoolFilters))
            .take(&wrong),
        "wrong original occurrence is not the emitted slot"
    );
    wrong = requirement(3);
    wrong.token = FloatToken::Double(0x7ff8_0000_0000_0017);
    assert!(
        !indexed
            .for_record(7, Some(Kind::LootPoolFilters))
            .take(&wrong),
        "different IEEE width is not covered"
    );
    wrong = requirement(3);
    wrong.token = FloatToken::Single(0x7fc0_0018);
    assert!(
        !indexed
            .for_record(7, Some(Kind::LootPoolFilters))
            .take(&wrong),
        "different source payload bits are not covered"
    );
    wrong = requirement(3);
    wrong.source_type = "OtherSource".to_owned();
    assert!(
        !indexed
            .for_record(7, Some(Kind::LootPoolFilters))
            .take(&wrong),
        "different actual owning struct is not covered"
    );
    wrong = requirement(3);
    wrong.field = "filter.other".to_owned();
    assert!(
        !indexed
            .for_record(7, Some(Kind::LootPoolFilters))
            .take(&wrong),
        "the original diagnostic path is still exact"
    );
    let mut coverage = indexed.for_record(7, Some(Kind::LootPoolFilters));
    assert!(
        coverage.take(&requirement(3)),
        "unmutated control is covered"
    );
    assert!(
        !coverage.take(&requirement(3)),
        "an extra same-slot requirement remains uncovered"
    );
}

#[test]
fn named_contracts_retain_their_old_path_and_once_per_slot_behavior() {
    let mut value = captured(3);
    value.occurrence = NumberOccurrence::NamedField;
    let rows = rows(Kind::QualityDistributions, vec![value]);
    let targets = targets(Kind::QualityDistributions, 7);
    let indexed = NumberCaptures::new(Some(&rows), Some(&targets));
    let mut required = requirement(99);
    required.contract = Some(ResourceNumberContract::Number);
    let mut coverage = indexed.for_record(7, Some(Kind::QualityDistributions));
    assert!(
        coverage.take(&required),
        "named coverage still uses its established owner/path contract"
    );
    required.original_ordinal = 100;
    assert!(
        !coverage.take(&required),
        "duplicate names still cannot consume one named receipt twice"
    );
    required.contract = None;
    assert!(
        !indexed
            .for_record(7, Some(Kind::QualityDistributions))
            .take(&required),
        "unknown non-Loot contracts remain blocking"
    );
    assert!(
        !indexed
            .for_record(7, Some(Kind::LootPoolFilters))
            .take(&required),
        "a named receipt does not impersonate a complete Loot occurrence"
    );
}
