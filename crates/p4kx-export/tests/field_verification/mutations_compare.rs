// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::json;

use super::compare::{self, Class, Rule, Summary};
use super::units::{Normalization, Unit};

#[test]
fn every_field_result_class_is_distinct_and_both_missing_is_never_equal() {
    let left = json!(0.1);
    let same = json!(0.1);
    let different = json!(0.2);
    let object = json!({"value": 0.1});
    for (reference, catalog, rule, expected) in [
        (Some(&left), Some(&same), Rule::F32RoundTrip, Class::Equal),
        (
            Some(&left),
            Some(&different),
            Rule::F32RoundTrip,
            Class::Different,
        ),
        (Some(&left), None, Rule::Exact, Class::ReferenceOnly),
        (None, Some(&same), Rule::Exact, Class::P4kxOnly),
        (None, None, Rule::Exact, Class::Missing),
        (Some(&object), Some(&object), Rule::Exact, Class::Unhandled),
        (
            Some(&left),
            Some(&same),
            Rule::Derived {
                reason: "producer formula".to_owned(),
            },
            Class::Derived,
        ),
        (
            Some(&left),
            Some(&same),
            Rule::Curated {
                reason: "reviewed override".to_owned(),
            },
            Class::Derived,
        ),
    ] {
        assert_eq!(
            compare::values(
                reference,
                catalog,
                &Unit::Dimensionless,
                &Unit::Dimensionless,
                &rule
            ),
            expected,
            "each missing/derived/raw outcome has its own accounting class"
        );
    }
}

#[test]
fn noncomparative_outcomes_never_manufacture_independent_evidence() {
    let mut summary = Summary::default();
    for class in [Class::Missing, Class::Derived, Class::Excluded] {
        summary.record(class);
    }
    assert_eq!(
        summary.compared(),
        0,
        "missing and derived states are not comparisons"
    );
    assert!(
        !summary.succeeds(),
        "zero independent comparisons never pass"
    );
    summary.record(Class::Equal);
    assert!(
        summary.succeeds(),
        "one actual equal comparison can succeed"
    );
    summary.record(Class::Unhandled);
    assert!(
        !summary.succeeds(),
        "an unhandled populated value blocks a pass"
    );
}

#[test]
fn f32_round_trip_tolerance_is_source_precision_not_a_broad_relative_allowance() {
    let reference = json!(0.1);
    let stored = json!(f64::from(0.1_f32));
    let changed = json!(0.100_001);
    assert_eq!(
        compare::values(
            Some(&reference),
            Some(&stored),
            &Unit::Seconds,
            &Unit::Seconds,
            &Rule::F32RoundTrip
        ),
        Class::Equal,
        "decimal serialization within the same f32 rounding cell agrees"
    );
    assert_eq!(
        compare::values(
            Some(&reference),
            Some(&changed),
            &Unit::Seconds,
            &Unit::Seconds,
            &Rule::F32RoundTrip
        ),
        Class::Different,
        "a real changed source f32 is detected"
    );
    let large = json!((1_u64 << 60) + 1);
    let other = json!((1_u64 << 60) + 2);
    assert_eq!(
        compare::values(
            Some(&large),
            Some(&other),
            &Unit::Dimensionless,
            &Unit::Dimensionless,
            &Rule::Exact
        ),
        Class::Different,
        "large integers never lose identity through f64 conversion"
    );
}

#[test]
fn incompatible_resource_units_require_an_explicit_reviewed_normalization() {
    let million = json!(1_000_000);
    let one = json!(1);
    assert_eq!(
        compare::values(
            Some(&million),
            Some(&one),
            &Unit::MicroScu,
            &Unit::Scu,
            &Rule::F32RoundTrip
        ),
        Class::Uncomparable,
        "unit mismatch never becomes numerical agreement"
    );
    let rule = Rule::Normalized {
        normalization: Normalization::MicroScuToScu,
        reason: "1 SCU equals 10^6 microSCU".to_owned(),
    };
    assert_eq!(
        compare::values(
            Some(&million),
            Some(&one),
            &Unit::MicroScu,
            &Unit::Scu,
            &rule
        ),
        Class::Equal,
        "the declared unit conversion is applied"
    );
    assert_eq!(
        compare::values(
            Some(&one),
            Some(&one),
            &Unit::Resource {
                guid: super::fixtures::GUID.to_owned(),
                representation: super::units::ResourceRepresentation::StandardUnits
            },
            &Unit::Resource {
                guid: super::fixtures::OTHER.to_owned(),
                representation: super::units::ResourceRepresentation::StandardUnits
            },
            &Rule::F32RoundTrip
        ),
        Class::Uncomparable,
        "different resource identities are incompatible units"
    );
}

#[test]
fn standard_resource_units_and_power_segments_are_not_interchangeable() {
    let value = json!(1);
    let standard = Unit::Resource {
        guid: super::fixtures::RESOURCE.to_owned(),
        representation: super::units::ResourceRepresentation::StandardUnits,
    };
    let segments = Unit::Resource {
        guid: super::fixtures::RESOURCE.to_owned(),
        representation: super::units::ResourceRepresentation::PowerSegments,
    };
    assert_eq!(
        compare::values(
            Some(&value),
            Some(&value),
            &standard,
            &segments,
            &Rule::F32RoundTrip
        ),
        Class::Uncomparable,
        "equal numbers for the same ResourceType still require compatible stored unit representations"
    );
}
