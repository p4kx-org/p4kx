// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{
    Kind, NumberCaptures, NumberOccurrence, NumberRequirement, captured, requirement, rows, targets,
};

#[test]
fn both_mining_families_cover_exact_original_raw_occurrences_once() {
    for kind in [Kind::RefiningProcesses, Kind::MiningGlobalParams] {
        let mut actual = captured(3);
        actual.occurrence = NumberOccurrence::AuthoredDefinitionRawValue { ordinal: 3 };
        actual.field = "unprojectedNumber".to_owned();
        actual.source_type = kind.records().type_name.to_owned();
        let rows = rows(kind, vec![actual]);
        let targets = targets(kind, 7);
        let indexed = NumberCaptures::new(Some(&rows), Some(&targets));
        let mut coverage = indexed.for_record(7, Some(kind));
        assert!(
            coverage.take(&required(kind, 3)),
            "the actual emitted original raw scalar is covered"
        );
        assert!(
            !coverage.take(&required(kind, 3)),
            "one physical receipt covers one actual occurrence only"
        );
        assert!(
            !indexed.for_record(7, Some(kind)).take(&required(kind, 4)),
            "another original ordinal is not covered by equal names/bits"
        );
        assert!(
            !indexed.for_record(8, Some(kind)).take(&required(kind, 3)),
            "another physical owner is not covered"
        );
        assert!(
            !indexed.for_record(7, None).take(&required(kind, 3)),
            "the owning family remains explicit"
        );
    }
}

#[test]
fn mining_raw_coverage_keeps_width_bits_type_path_and_emission_checks() {
    for kind in [Kind::RefiningProcesses, Kind::MiningGlobalParams] {
        let mut actual = captured(3);
        actual.occurrence = NumberOccurrence::AuthoredDefinitionRawValue { ordinal: 3 };
        actual.field = "unprojectedNumber".to_owned();
        actual.source_type = kind.records().type_name.to_owned();
        let rows = rows(kind, vec![actual]);
        let targets = targets(kind, 7);
        let indexed = NumberCaptures::new(Some(&rows), Some(&targets));
        let mut wrong = required(kind, 3);
        wrong.token = super::FloatToken::Double(0x7ff8_0000_0000_0017);
        assert!(
            !indexed.for_record(7, Some(kind)).take(&wrong),
            "the original IEEE width remains mandatory"
        );
        wrong = required(kind, 3);
        wrong.token = super::FloatToken::Single(0x7fc0_0018);
        assert!(
            !indexed.for_record(7, Some(kind)).take(&wrong),
            "the actual source payload bits remain mandatory"
        );
        wrong = required(kind, 3);
        wrong.source_type = "AnotherOriginal".to_owned();
        assert!(
            !indexed.for_record(7, Some(kind)).take(&wrong),
            "the exact source owner type remains mandatory"
        );
        wrong = required(kind, 3);
        wrong.field = "otherOriginal".to_owned();
        assert!(
            !indexed.for_record(7, Some(kind)).take(&wrong),
            "the actual logical field path remains mandatory"
        );
        let missing = super::targets(kind, 8);
        assert!(
            !NumberCaptures::new(Some(&rows), Some(&missing))
                .for_record(7, Some(kind))
                .take(&required(kind, 3)),
            "unemitted original receipts cannot grant health"
        );
    }
}

fn required(kind: Kind, ordinal: u64) -> NumberRequirement {
    let mut value = requirement(ordinal);
    value.field = "unprojectedNumber".to_owned();
    value.source_type = kind.records().type_name.to_owned();
    value
}
