// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{craft_time, whole};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const POSITIVE_LIMIT: f64 = 9_223_372_036_854_775_808.0;
const NEGATIVE_LIMIT: f64 = -POSITIVE_LIMIT;
const SECONDS_PER_DAY: i64 = 86_400;

fn data(days: i64, seconds: f64, single: f32) -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Time")
                .field("days", TestType::Int64)
                .field("hours", TestType::Int32)
                .field("minutes", TestType::Int32)
                .field("seconds", TestType::Double)
                .field("single", TestType::Single),
        )
        .record(
            TestRecord::new(0x100, "Time", "Time.Original", "time.xml")
                .value("days", TestValue::Int64(days))
                .value("hours", TestValue::Int32(0))
                .value("minutes", TestValue::Int32(0))
                .value("seconds", TestValue::Double(seconds))
                .value("single", TestValue::Single(single)),
        )
        .build()
}

#[test]
fn finite_large_float_components_round_within_the_i64_range() -> TestResult {
    let source = 1_000_000_000.6;
    let single = 1_000_000_064.0;
    let bytes = data(0, source, single);
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("actual duration Record")?;
    let tree = record.tree()?;
    assert!(tree.is_from(&record));
    let root = tree.root();
    assert_eq!(root.field("seconds"), Some(&Value::Double(source)));
    assert_eq!(root.field("single"), Some(&Value::Single(single)));
    assert_eq!(whole(Some(root), "seconds"), Some(1_000_000_001));
    assert_eq!(whole(Some(root), "single"), Some(1_000_000_064));
    assert_eq!(craft_time(Some(root)), Some(1_000_000_001));
    Ok(())
}

#[test]
fn rounded_float_conversion_keeps_the_true_i64_boundaries() -> TestResult {
    for (source, expected) in [
        (POSITIVE_LIMIT, None),
        (NEGATIVE_LIMIT.next_down(), None),
        (f64::INFINITY, None),
        (POSITIVE_LIMIT.next_down(), Some(i64::MAX - 1023)),
        (NEGATIVE_LIMIT, Some(i64::MIN)),
    ] {
        let bytes = data(0, source, 0.0);
        let forge = DataForge::open(&bytes)?;
        let record = forge.records().next().ok_or("actual duration Record")?;
        let tree = record.tree()?;
        assert_eq!(tree.root().field("seconds"), Some(&Value::Double(source)));
        assert_eq!(
            whole(Some(tree.root()), "seconds"),
            expected,
            "source {source}"
        );
    }
    Ok(())
}

#[test]
fn actual_checked_duration_multiply_and_sum_overflow_remain_unrepresentable() -> TestResult {
    let days = i64::MAX / SECONDS_PER_DAY;
    let rest = i32::try_from(i64::MAX % SECONDS_PER_DAY)? + 1;
    assert!(days.checked_mul(SECONDS_PER_DAY).is_some());
    assert!(
        days.checked_mul(SECONDS_PER_DAY)
            .and_then(|value| value.checked_add(i64::from(rest)))
            .is_none()
    );
    assert!((days + 1).checked_mul(SECONDS_PER_DAY).is_none());
    for (days, seconds) in [(days, f64::from(rest)), (days + 1, 0.0)] {
        let bytes = data(days, seconds, 0.0);
        let forge = DataForge::open(&bytes)?;
        let record = forge.records().next().ok_or("actual duration Record")?;
        let tree = record.tree()?;
        assert_eq!(tree.root().field("days"), Some(&Value::Int64(days)));
        assert_eq!(tree.root().field("seconds"), Some(&Value::Double(seconds)));
        assert_eq!(craft_time(Some(tree.root())), None);
    }
    Ok(())
}
