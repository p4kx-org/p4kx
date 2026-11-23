// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Mutex;

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{
    FieldStatistics, FieldStatisticsInput, FieldStatisticsMetricCoverage, FieldStatisticsReduction,
    FieldStatisticsSourceScope, FieldStatisticsUse,
};
use crate::{DataForge, RecordOrderingName};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn all_record_census_preserves_omitted_headers_without_counting_them() -> TestResult {
    let mut bytes = fixture(None);
    let start = {
        let forge = DataForge::open(&bytes)?;
        let record = forge.record(0).ok_or("the original omitted header")?;
        let span = forge
            .original_text_span(record.stored_file_name()?)
            .ok_or("original stored filepath bytes")?;
        usize::try_from(span.start)?
    };
    *bytes
        .get_mut(start)
        .ok_or("the original file path starts inside the fixture")? = 0xff;
    let forge = DataForge::open(&bytes)?;
    let mut census = Vec::new();
    let reduction =
        FieldStatistics::of_all_with_reduction(&forge, |source| census.push(source), |_| {})?;
    assert_eq!(
        reduction.statistics(),
        &FieldStatistics::of_all(&forge)?,
        "exact legacy all-Record results"
    );
    assert_eq!(
        reduction.statistics().records(),
        1,
        "only readable file paths are scanned"
    );
    let [omitted, admitted] = census.as_slice() else {
        return Err("the census requires both original physical headers".into());
    };
    assert_eq!(
        omitted.record_index, 0,
        "the missing path retains its exact header identity"
    );
    assert!(
        !omitted.indexed && omitted.file_name.is_none(),
        "no invented original path bytes"
    );
    assert_eq!(
        admitted.record_index, 1,
        "the admitted original physical header"
    );
    assert!(
        admitted.indexed && admitted.file_name.is_some(),
        "the actual admitted filename is bound"
    );
    assert_eq!(
        admitted.name_state,
        RecordOrderingName::Absent,
        "an authored absent ordering name is not a failure"
    );
    assert_eq!(
        reduction.source_scope(),
        FieldStatisticsSourceScope::AllRecordPathsObserved {
            unreadable_paths: 1,
            unreadable_names: 0
        },
        "the exact physical census separately records the unreadable filename"
    );
    Ok(())
}

#[test]
fn unreadable_array_and_scalar_damage_only_their_eligible_metrics() -> TestResult {
    let mut bytes = fixture(None);
    damage_owners(&mut bytes)?;
    let forge = DataForge::open(&bytes)?;
    let reduced = FieldStatistics::of_records_with_reduction(&forge, &[0, 1], |_| {})?;
    assert_eq!(
        reduced.statistics(),
        &FieldStatistics::of_records(&forge, &[0, 1])?,
        "known aggregate values are unchanged"
    );

    let bad = coverage(&reduced, "bad")?;
    assert!(
        bad.structure.is_covered() && bad.empty.is_covered() && bad.extrema.is_covered(),
        "a Boolean decoding error still supplies its known default shape"
    );
    assert!(
        !bad.distinct.is_covered(),
        "the unreadable Boolean may supply a missing fingerprint"
    );

    let area = coverage(&reduced, "members")?;
    let child = coverage(&reduced, "members/value")?;
    assert_eq!(
        area, child,
        "the unresolved class array affects only its own subtree"
    );
    assert!(
        !area.structure.is_covered() && !area.extrema.is_covered() && !area.empty.is_covered(),
        "missing array children leave the affected path incomplete"
    );

    let sibling = coverage(&reduced, "unrelated")?;
    assert!(
        sibling.structure.is_covered()
            && sibling.empty.is_covered()
            && sibling.distinct.is_covered()
            && sibling.extrema.is_covered(),
        "the separate scalar path is still covered"
    );

    let missing = FieldStatistics::of_records_with_reduction(&forge, &[0, 1, u32::MAX], |_| {})?;
    assert!(
        coverage(&missing, "unrelated")?.structure.is_covered(),
        "a known absent requested index has no hidden fields"
    );
    let bytes = fixture(Some(u16::MAX));
    let unreadable = DataForge::open(&bytes)?;
    let with_unknown = FieldStatistics::of_records_with_reduction(&unreadable, &[0, 1], |_| {})?;
    assert!(
        !coverage(&with_unknown, "unrelated")?.structure.is_covered(),
        "an unreadable original root can obscure any admitted field path"
    );
    Ok(())
}

fn coverage(
    result: &FieldStatisticsReduction<'_>,
    path: &str,
) -> Result<FieldStatisticsMetricCoverage, Box<dyn Error>> {
    let position = result
        .statistics()
        .fields()
        .iter()
        .position(|row| row.path() == path)
        .ok_or("an actual canonical field row")?;
    let row = result
        .row(position)
        .ok_or("the associated structural coverage")?;
    Ok(row.metric_coverage())
}

fn damage_owners(bytes: &mut [u8]) -> TestResult {
    let observations = Mutex::new(Vec::new());
    {
        let forge = DataForge::open(bytes)?;
        FieldStatistics::of_records_observed(&forge, &[0], |event| {
            if matches!(event.usage(), FieldStatisticsUse::Consulted)
                && let FieldStatisticsInput::Slot(slot) = event.input()
                && let Some(by) = slot.bytes.clone()
            {
                observations
                    .lock()
                    .expect("the test owns the physical receipt")
                    .push((
                        event.leaf_to_root().next().unwrap_or_default().to_owned(),
                        by,
                    ));
            }
        })?;
    }
    for (name, range) in observations.into_inner()? {
        if name == "bad" {
            let index = usize::try_from(range.start)?;
            *bytes.get_mut(index).ok_or("the Boolean slot start")? = 2;
        }
        if name == "members" {
            let end = usize::try_from(range.end)?;
            let start = end
                .checked_sub(size_of::<u32>())
                .ok_or("the array first-index word")?;
            bytes
                .get_mut(start..end)
                .ok_or("the original array header")?
                .copy_from_slice(&u32::MAX.to_le_bytes());
        }
    }
    Ok(())
}

fn fixture(unreadable_instance: Option<u16>) -> Vec<u8> {
    let mut schema = DataForgeBuilder::new()
        .structure(TestStruct::new("Member").field("value", TestType::Int32))
        .structure(
            TestStruct::new("Owner")
                .field("bad", TestType::Bool)
                .array("members", TestType::Class("Member".to_owned()))
                .field("unrelated", TestType::Int32),
        );
    for (id, path) in [(1, "libs/one.xml"), (2, "libs/two.xml")] {
        let mut record = TestRecord::new(id, "Owner", "Fixture", path)
            .value("bad", TestValue::Bool(false))
            .value("members", children())
            .value("unrelated", TestValue::Int32(7));
        if id == 1 {
            if let Some(instance) = unreadable_instance {
                record = record.with_instance_index(instance);
            }
        } else {
            record = record.without_name();
        }
        schema = schema.record(record);
    }
    schema.build()
}

fn children() -> TestValue {
    TestValue::Array(vec![TestValue::Struct(
        TestInstance::new("Member").value("value", TestValue::Int32(3)),
    )])
}
