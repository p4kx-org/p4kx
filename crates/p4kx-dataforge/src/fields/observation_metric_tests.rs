// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Mutex;

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{
    FieldStatistics, FieldStatisticsDistinctUse, FieldStatisticsGap, FieldStatisticsUse,
    FieldStatisticsValue, MAX_DISTINCT_VALUES, Number,
};
use crate::DataForge;
use crate::fields::FieldStatisticsInput;

type TestResult = Result<(), Box<dyn Error>>;
type Events = Vec<(FieldStatisticsInput, FieldStatisticsUse)>;

fn fixture() -> Vec<u8> {
    let values = [0, 0]
        .into_iter()
        .chain(1..=33)
        .map(TestValue::Int32)
        .collect();
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Metrics")
                .array("values", TestType::Int32)
                .field("valid", TestType::Bool)
                .field("label", TestType::String),
        )
        .record(
            TestRecord::new(7, "Metrics", "Test", "libs/statistics.xml")
                .value("values", TestValue::Array(values))
                .value("valid", TestValue::Bool(false))
                .value("label", TestValue::text("literal")),
        )
        .build()
}

fn scan(source: &[u8], records: &[u32]) -> Result<(FieldStatistics, Events), Box<dyn Error>> {
    let forge = DataForge::open(source)?;
    let events = Mutex::new(Vec::new());
    let stats = FieldStatistics::of_records_observed(&forge, records, |event| {
        let usage = event.usage().clone();
        events
            .lock()
            .expect("one test owns the stream")
            .push((event.into_input(), usage));
    })?;
    assert_eq!(
        stats,
        FieldStatistics::of_records(&forge, records)?,
        "observing operations does not change the ordinary aggregate"
    );
    Ok((stats, events.into_inner()?))
}

#[test]
fn every_numeric_input_keeps_its_range_and_actual_distinct_decision() -> TestResult {
    let source = fixture();
    let (statistics, events) = scan(&source, &[0])?;
    let field = statistics
        .fields()
        .iter()
        .find(|field| field.path() == "values")
        .ok_or("numeric row")?;
    assert_eq!(
        field.distinct(),
        MAX_DISTINCT_VALUES,
        "the configured count is retained"
    );
    assert!(
        field.distinct_capped(),
        "the original reducer sees an additional distinct value"
    );
    assert_eq!(
        field.min(),
        Some(Number::Integer(0)),
        "the numeric minimum remains exact"
    );
    assert_eq!(
        field.max(),
        Some(Number::Integer(33)),
        "values after the cap still affect maxima"
    );
    check_numeric(&source, &events)
}

fn check_numeric(source: &[u8], events: &Events) -> TestResult {
    use FieldStatisticsDistinctUse::{Admitted, AfterCap, ComparedDuplicate, ReachedCap};
    let mut outcome = Vec::new();
    for (site, usage) in events {
        let (
            FieldStatisticsInput::ArrayElement {
                index,
                bytes: Some(bytes),
                ..
            },
            FieldStatisticsUse::Counted {
                value:
                    FieldStatisticsValue::Nonempty {
                        distinct,
                        number: Some(Number::Integer(number)),
                        ..
                    },
                ..
            },
        ) = (site, usage)
        else {
            continue;
        };
        let range = usize::try_from(bytes.start)?..usize::try_from(bytes.end)?;
        assert_eq!(
            source.get(range).ok_or("admitted original value extent")?,
            i32::try_from(*number)?.to_le_bytes(),
            "the numeric contribution resolves to its original Int32 bytes"
        );
        outcome.push((*index, *distinct));
    }
    assert_eq!(
        outcome.len(),
        35,
        "every consumed numeric candidate is reported"
    );
    for (index, usage) in [
        (0, Admitted),
        (1, ComparedDuplicate),
        (33, ReachedCap),
        (34, AfterCap),
    ] {
        assert!(
            outcome.contains(&(index, usage)),
            "the assigned distinct outcome for element {index}"
        );
    }
    Ok(())
}

#[test]
fn a_bad_value_counts_as_shape_and_has_a_separate_attempt() -> TestResult {
    let mut source = fixture();
    let (_, before) = scan(&source, &[0])?;
    let site = before
        .into_iter()
        .find_map(|(site, usage)| match (site, usage) {
            (FieldStatisticsInput::Slot(slot), FieldStatisticsUse::Consulted)
                if slot.slot_position == 1 =>
            {
                Some(slot)
            }
            _ => None,
        })
        .ok_or("observed Boolean slot")?;
    let start = usize::try_from(site.bytes.as_ref().ok_or("Boolean original bytes")?.start)?;
    *source.get_mut(start).ok_or("Boolean position in fixture")? = 2;
    let (statistics, events) = scan(&source, &[0, u32::MAX])?;
    assert_eq!(
        statistics.unreadable_records(),
        1,
        "only the requested absent Record is refused"
    );
    assert_eq!(
        statistics.unreadable_values(),
        1,
        "only the malformed Boolean is skipped"
    );
    let site = FieldStatisticsInput::Slot(site);
    assert!(
        events.iter().any(|(input, usage)| input == &site
            && matches!(
                usage,
                FieldStatisticsUse::Counted {
                    value: FieldStatisticsValue::Unreadable,
                    ..
                }
            )),
        "the malformed value still contributes its actual structural occurrence"
    );
    assert!(
        events.contains(&(
            site,
            FieldStatisticsUse::Skipped(FieldStatisticsGap::UnreadableValue)
        )),
        "the same original field is independently recorded as an unreadable operand"
    );
    assert!(
        events.contains(&(
            FieldStatisticsInput::Record(u32::MAX),
            FieldStatisticsUse::Skipped(FieldStatisticsGap::MissingRecord)
        )),
        "a nonexistent Record is a refusal at the requested position"
    );
    assert_text(&source, &events)
}

fn assert_text(source: &[u8], events: &Events) -> TestResult {
    let text = events
        .iter()
        .find_map(|(_, usage)| match usage {
            FieldStatisticsUse::Counted {
                value:
                    FieldStatisticsValue::Nonempty {
                        number: None,
                        text: Some(text),
                        ..
                    },
                ..
            } => Some(text),
            _ => None,
        })
        .ok_or("retained text span")?;
    let range = usize::try_from(text.start)?..usize::try_from(text.end)?;
    assert_eq!(
        source.get(range),
        Some(b"literal".as_slice()),
        "original decoded text-table bytes"
    );
    Ok(())
}
