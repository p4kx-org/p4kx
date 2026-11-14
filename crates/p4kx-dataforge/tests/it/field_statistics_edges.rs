// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Field statistics of Records that are odd: References, weak pointers, enum and locale values,
//! an instance owned twice, complete large Records and a DataForge with too many paths.

use p4kx_dataforge::{
    DataForge, FieldStat, FieldStatistics, FieldStatisticsError, MAX_FIELD_PATHS, Number,
};
use p4kx_testkit::{
    DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue,
    many_paths_database,
};

use crate::field_common::{TestResult, ship, stat};

const SAMPLE: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;

fn raw(words: [u32; 2]) -> TestValue {
    TestValue::Raw(words.iter().flat_map(|word| word.to_le_bytes()).collect())
}

fn kind_counts(stat: &FieldStat) -> Vec<String> {
    stat.kinds()
        .iter()
        .map(|(kind, count)| format!("{kind} {count}"))
        .collect()
}

#[test]
fn a_reference_counts_the_distinct_records_it_links_to() -> TestResult {
    let bytes = crate::field_common::schema()
        .record(ship(1, "ships/one.xml").value("link", TestValue::Reference(0xaa)))
        .record(ship(2, "ships/two.xml").value("link", TestValue::Reference(0xaa)))
        .record(ship(3, "ships/three.xml").value("link", TestValue::Reference(0xbb)))
        .record(ship(4, "ships/four.xml"))
        .build();
    let forge = DataForge::open(&bytes)?;

    let statistics = FieldStatistics::of_all(&forge)?;

    let link = stat(&statistics, "link")?;
    assert_eq!(kind_counts(link), ["Reference 4"], "kind");
    assert_eq!(link.empty(), 1, "the null Reference");
    assert_eq!(link.distinct(), 2, "aa and bb, the null one is no value");
    assert!(!link.distinct_capped(), "below the cap");
    assert_eq!(link.min(), None, "a GUID is no number");
    Ok(())
}

#[test]
fn pointers_and_inline_structs_have_no_distinct_values() -> TestResult {
    let bytes = crate::field_common::database();
    let forge = DataForge::open(&bytes)?;

    let statistics = FieldStatistics::of_all(&forge)?;

    for path in ["anchor", "position", "Components"] {
        assert_eq!(stat(&statistics, path)?.distinct(), 0, "{path}");
    }
    Ok(())
}

#[test]
fn weak_pointers_count_null_dangling_and_valid_ones() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Wheel").field("size", TestType::Int8))
        .structure(TestStruct::new("Car").field("front", TestType::WeakPointer("Wheel".to_owned())))
        .instance("wheel", TestInstance::new("Wheel"))
        .record(
            TestRecord::new(1, "Car", "Car.One", "cars/one.xml")
                .value("front", TestValue::pointer("wheel")),
        )
        .record(TestRecord::new(2, "Car", "Car.Two", "cars/two.xml"))
        .record(
            TestRecord::new(3, "Car", "Car.Three", "cars/three.xml").value("front", raw([99, 0])),
        )
        .build();
    let forge = DataForge::open(&bytes)?;

    let statistics = FieldStatistics::of_all(&forge)?;

    let front = stat(&statistics, "front")?;
    assert_eq!(kind_counts(front), ["WeakPointer 3"], "kind");
    assert_eq!(front.occurrences(), 3, "every Record has the field");
    assert_eq!(front.empty(), 1, "the null pointer");
    assert_eq!(front.distinct(), 0, "a pointer has no distinct values");
    assert_eq!(statistics.unreadable_values(), 1, "the dangling pointer");
    assert_eq!(statistics.unreadable_records(), 0, "the Records read");
    assert!(
        statistics.children("front").next().is_none(),
        "a weak pointer owns nothing"
    );
    Ok(())
}

#[test]
fn enum_and_locale_values_are_texts() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .enumeration("Grade", ["civilian", "military"])
        .structure(
            TestStruct::new("Part")
                .field("grade", TestType::Enum("Grade".to_owned()))
                .field("label", TestType::Locale),
        )
        .record(
            TestRecord::new(1, "Part", "Part.One", "parts/one.xml")
                .value("grade", TestValue::text("civilian"))
                .value("label", TestValue::text("@item_name_a")),
        )
        .record(
            TestRecord::new(2, "Part", "Part.Two", "parts/two.xml")
                .value("grade", TestValue::text("military"))
                .value("label", TestValue::text("@item_name_a")),
        )
        .record(
            TestRecord::new(3, "Part", "Part.Three", "parts/three.xml")
                .value("grade", TestValue::text("civilian"))
                .value("label", TestValue::text("")),
        )
        .build();
    let forge = DataForge::open(&bytes)?;

    let statistics = FieldStatistics::of_all(&forge)?;

    let grade = stat(&statistics, "grade")?;
    assert_eq!(kind_counts(grade), ["Enum 3"], "enum kind");
    assert_eq!((grade.empty(), grade.distinct()), (0, 2), "two options");
    assert_eq!(grade.min(), None, "an option is no number");
    let label = stat(&statistics, "label")?;
    assert_eq!(kind_counts(label), ["Locale 3"], "locale kind");
    assert_eq!(
        (label.empty(), label.distinct()),
        (1, 1),
        "one key, one empty"
    );
    Ok(())
}

#[test]
fn an_instance_owned_twice_is_counted_once_and_reported() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Node")
                .field("next", TestType::StrongPointer("Node".to_owned()))
                .field("mark", TestType::Int8),
        )
        .instance(
            "a",
            TestInstance::new("Node").value("next", TestValue::pointer("b")),
        )
        .instance(
            "b",
            TestInstance::new("Node")
                .value("next", TestValue::pointer("a"))
                .value("mark", TestValue::Int8(2)),
        )
        .record(
            TestRecord::new(SAMPLE, "Node", "Node.One", "node.xml")
                .value("next", TestValue::pointer("a"))
                .value("mark", TestValue::Int8(1)),
        )
        .build();
    let forge = DataForge::open(&bytes)?;

    let statistics = FieldStatistics::of_all(&forge)?;

    assert_eq!(statistics.unreadable_values(), 1, "the loop back to a");
    assert_eq!(statistics.unreadable_records(), 0, "the Record reads");
    assert_eq!(
        stat(&statistics, "next/Node/next/Node/next")?.occurrences(),
        1,
        "the pointer that closes the loop is a value, the instance behind it is not read again"
    );
    assert!(
        statistics
            .children("next/Node/next/Node/next")
            .next()
            .is_none(),
        "nothing below it"
    );
    Ok(())
}

fn large_database(elements: usize) -> Vec<u8> {
    let fields = 29_959;
    let mut big = TestStruct::new("Big");
    for field in 0..fields {
        big = big.field(format!("b{field}"), TestType::Int32);
    }
    let bigs = |count: usize| {
        TestValue::Array(
            (0..count)
                .map(|_| TestValue::Struct(TestInstance::new("Big")))
                .collect(),
        )
    };
    DataForgeBuilder::new()
        .structure(big)
        .structure(
            TestStruct::new("Holder")
                .field("before", TestType::Int32)
                .array("bigs", TestType::Class("Big".to_owned())),
        )
        .record(
            TestRecord::new(1, "Holder", "Holder.Small", "holders/small.xml")
                .value("before", TestValue::Int32(5))
                .value("bigs", bigs(1)),
        )
        .record(
            TestRecord::new(2, "Holder", "Holder.Large", "holders/large.xml")
                .value("before", TestValue::Int32(7))
                .value("bigs", bigs(elements)),
        )
        .build()
}

#[test]
fn a_large_valid_record_is_counted_completely_and_atomically() -> TestResult {
    let bytes = large_database(18);
    let forge = DataForge::open(&bytes)?;
    let source = forge
        .structs()
        .find(|value| value.name().is_ok_and(|name| name == "Big"))
        .ok_or("the original Big declaration")?;
    assert_eq!(
        source.properties().count(),
        29_959,
        "the actual original field count"
    );
    assert_eq!(source.size(), 119_836, "the original Int32 storage size");
    assert_eq!(
        source.instance_count(),
        19,
        "all original small and large array elements exist"
    );
    assert_eq!(forge.records().len(), 2, "both original owners exist");
    let statistics = FieldStatistics::of_all(&forge)?;
    assert_eq!(
        statistics.records(),
        2,
        "both original Records are examined"
    );
    assert_eq!(
        statistics.unreadable_records(),
        0,
        "both valid Records are fully counted"
    );
    let before = stat(&statistics, "before")?;
    assert_eq!(
        before.occurrences(),
        2,
        "both complete source values contribute"
    );
    assert_eq!(before.records(), 2, "both original records contribute");
    assert_eq!(
        before.max(),
        Some(Number::Integer(7)),
        "the final original scalar contributes"
    );
    let bigs = stat(&statistics, "bigs")?;
    assert_eq!(
        bigs.occurrences(),
        19,
        "all original array elements survive"
    );
    assert_eq!(
        stat(&statistics, "bigs/b0")?.occurrences(),
        19,
        "each original field occurrence survives"
    );
    assert_eq!(
        statistics.unreadable_values(),
        0,
        "no source values are discarded"
    );
    Ok(())
}

#[test]
fn a_second_large_valid_record_shape_is_counted() -> TestResult {
    let bytes = large_database(17);
    let forge = DataForge::open(&bytes)?;

    let statistics = FieldStatistics::of_all(&forge)?;

    assert_eq!(statistics.unreadable_records(), 0, "both Records read");
    assert_eq!(stat(&statistics, "before")?.occurrences(), 2, "both count");
    Ok(())
}

#[test]
fn more_paths_than_the_limit_are_an_error_on_any_number_of_threads() -> TestResult {
    let bytes = many_paths_database(33, 1_000);
    let forge = DataForge::open(&bytes)?;

    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()?;
        let result = pool.install(|| FieldStatistics::of_all(&forge));
        assert!(
            matches!(
                result,
                Err(FieldStatisticsError::TooManyPaths { limit }) if limit == MAX_FIELD_PATHS
            ),
            "{threads} threads: {:?}",
            result.map(|statistics| statistics.fields().len())
        );
    }
    Ok(())
}
