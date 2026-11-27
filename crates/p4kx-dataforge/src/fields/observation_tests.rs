// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ops::Range;
use std::sync::Mutex;

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{FieldStatistics, FieldStatisticsInput, FieldStatisticsSlot, FieldStatisticsUse};
use crate::DataForge;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn observed_statistics_keep_original_slot_and_array_positions() -> TestResult {
    let source = fixture();
    let forge = DataForge::open(&source)?;
    let (observed, sites) = scan(&forge)?;
    assert_eq!(
        observed,
        FieldStatistics::of_records(&forge, &[0])?,
        "observation preserves all ordinary statistics"
    );
    check_sites(&source, &sites)
}

fn fixture() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Base").field("scalar", TestType::Int32))
        .structure(
            TestStruct::new("Thing")
                .parent("Base")
                .array("numbers", TestType::Int32),
        )
        .record(
            TestRecord::new(19, "Thing", "Test", "libs/test.xml")
                .value("scalar", TestValue::Int32(17))
                .value(
                    "numbers",
                    TestValue::Array(vec![TestValue::Int32(23), TestValue::Int32(31)]),
                ),
        )
        .build()
}

type ScannedSites = Vec<(Vec<String>, FieldStatisticsInput)>;

fn scan<'source>(
    forge: &'source DataForge<'source>,
) -> Result<(FieldStatistics, ScannedSites), Box<dyn Error>> {
    let sites = Mutex::new(Vec::new());
    let observed = FieldStatistics::of_records_observed(forge, &[0], |site| {
        if !matches!(site.usage(), FieldStatisticsUse::Consulted) {
            return;
        }
        let path: Vec<_> = site.leaf_to_root().map(str::to_owned).collect();
        sites
            .lock()
            .expect("one fixture owns its collection")
            .push((path, site.into_input()));
    })?;
    Ok((observed, sites.into_inner()?))
}

fn check_sites(source: &[u8], sites: &[(Vec<String>, FieldStatisticsInput)]) -> TestResult {
    let [
        (_, FieldStatisticsInput::Slot(scalar)),
        (_, FieldStatisticsInput::Slot(array)),
        (
            first_path,
            FieldStatisticsInput::ArrayElement {
                header: first,
                index: 0,
                pool_index: 0,
                bytes: Some(a),
            },
        ),
        (
            second_path,
            FieldStatisticsInput::ArrayElement {
                header: second,
                index: 1,
                pool_index: 1,
                bytes: Some(b),
            },
        ),
    ] = sites
    else {
        return Err("one scalar, one array header and two physical elements were expected".into());
    };
    check_slots(source, scalar, array)?;
    assert_eq!(
        first_path,
        &["numbers"],
        "first element follows its original field"
    );
    assert_eq!(
        second_path, first_path,
        "the second element has the same path"
    );
    assert_eq!(
        first, array,
        "the first element retains its original header"
    );
    assert_eq!(
        second, array,
        "the second element retains its original header"
    );
    assert_eq!(
        bytes_at(source, a)?,
        23_i32.to_le_bytes(),
        "first value bytes"
    );
    assert_eq!(
        bytes_at(source, b)?,
        31_i32.to_le_bytes(),
        "second value bytes"
    );
    Ok(())
}

#[test]
fn rejected_array_header_does_not_create_physical_element_evidence() -> TestResult {
    let mut source = fixture();
    let header = {
        let forge = DataForge::open(&source)?;
        let (_, sites) = scan(&forge)?;
        sites
            .into_iter()
            .find_map(|(_, site)| match site {
                FieldStatisticsInput::Slot(slot) if slot.slot_position == 1 => slot.bytes,
                _ => None,
            })
            .ok_or("original array header unavailable")?
    };
    let end = usize::try_from(header.end)?;
    let start = end.checked_sub(4).ok_or("array first-index coordinate")?;
    source
        .get_mut(start..end)
        .ok_or("array slice missing")?
        .copy_from_slice(&u32::MAX.to_le_bytes());
    let forge = DataForge::open(&source)?;
    let (statistics, sites) = scan(&forge)?;
    assert_eq!(
        statistics.unreadable_values(),
        1,
        "only the broken array is skipped"
    );
    assert!(
        !sites
            .iter()
            .any(|(_, site)| matches!(site, FieldStatisticsInput::ArrayElement { .. })),
        "an invalid array header supplies no physical elements"
    );
    assert!(
        sites.iter().any(|(_, site)| matches!(
            site,
            FieldStatisticsInput::Slot(slot) if slot.bytes.as_ref() == Some(&header)
        )),
        "the original consulted header survives the refusal"
    );
    Ok(())
}

fn check_slots(
    source: &[u8],
    scalar: &FieldStatisticsSlot,
    header: &FieldStatisticsSlot,
) -> TestResult {
    assert_eq!(
        (
            scalar.record_index,
            scalar.owner_structure,
            scalar.owner_instance,
            scalar.layout_structure,
            scalar.slot_position
        ),
        (0, 1, 0, 1, 0),
        "the scalar is bound to the original physical authored occurrence"
    );
    assert_eq!(
        header.slot_position, 1,
        "the array header is at its inherited-first position"
    );
    assert_eq!(
        bytes_at(source, scalar.bytes.as_ref().ok_or("missing scalar span")?)?,
        17_i32.to_le_bytes(),
        "the scalar proof resolves actual original bytes"
    );
    assert_eq!(
        bytes_at(source, header.name.as_ref().ok_or("missing name span")?)?,
        b"numbers",
        "the array name proof resolves the original declaration text"
    );
    assert_eq!(
        bytes_at(source, header.bytes.as_ref().ok_or("missing array header")?)?,
        &[2, 0, 0, 0, 0, 0, 0, 0],
        "the same physical header supplies the authored count and first index"
    );
    Ok(())
}

fn bytes_at<'source>(
    source: &'source [u8],
    range: &Range<u64>,
) -> Result<&'source [u8], Box<dyn Error>> {
    let range = usize::try_from(range.start)?..usize::try_from(range.end)?;
    source
        .get(range)
        .ok_or_else(|| "observed bytes must belong to this source".into())
}
