// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::{
    DataForge, PositionedRecord, PositionedSources, PositionedValue, RecordJsonBytes,
    RecordJsonSource, TreeBuilder, Value,
};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SENTINEL: i32 = 0x0403_0201;

fn inherited_database() -> TestResult<Vec<u8>> {
    let mut input = DataForgeBuilder::new()
        .structure(TestStruct::new("Base").field("shared", TestType::Int32))
        .structure(
            TestStruct::new("Child")
                .parent("Base")
                .field("shared", TestType::Int32),
        )
        .record(
            TestRecord::new(42, "Child", "Child.One", "one.xml")
                .value("shared", TestValue::Int32(SENTINEL)),
        )
        .build();
    input
        .last_chunk_mut::<4>()
        .ok_or("derived field slot")?
        .copy_from_slice(&SENTINEL.to_le_bytes());
    Ok(input)
}

#[test]
fn positioned_inherited_fields_keep_distinct_equal_names() -> TestResult {
    let input = inherited_database()?;
    let forge = DataForge::open(&input)?;
    let record = forge.record(0).ok_or("original Record")?;
    let positioned = TreeBuilder::new().build_positioned(&record)?;
    assert_eq!(
        positioned.tree(),
        &record.tree()?,
        "positioning the same canonical read"
    );
    let fields = positioned.root_fields()?;
    let mut slots = Vec::new();
    for (ordinal, property) in [(0, 0), (1, 1)] {
        let site = fields.get(ordinal).ok_or("positioned declaration")?;
        assert_eq!(
            site.value(),
            &Value::Int32(SENTINEL),
            "equal authored values"
        );
        let name = site.field_name_sources().ok_or("name coordinates")?;
        assert!(name.complete(), "the name is fully observed");
        assert!(
            name.sources()
                .contains(&RecordJsonSource::PropertyName { index: property }),
            "exact inherited property"
        );
        let (_, raw) = bytes_of(&input, &name, RecordJsonBytes::Text)?;
        assert_eq!(raw, b"shared", "actual declared text");
        let value = site.value_sources();
        assert!(value.complete(), "direct scalar observation");
        assert!(
            value
                .sources()
                .contains(&RecordJsonSource::PropertyStorage {
                    index: property,
                    target_used: false
                }),
            "exact declaration used to decode"
        );
        let (offset, raw) = bytes_of(&input, &value, RecordJsonBytes::Slot)?;
        assert_eq!(raw, SENTINEL.to_le_bytes(), "actual scalar slot");
        slots.push(offset);
    }
    let [base, child] = slots.as_slice() else {
        return Err("two distinct slots".into());
    };
    assert_eq!(
        *base + 4,
        *child,
        "two distinct adjacent stored values with the same name"
    );
    assert!(fields.get(2).is_none(), "no fabricated third declaration");
    Ok(())
}

#[test]
fn positioned_nested_arrays_retain_their_own_header_and_selected_pool_slot() -> TestResult {
    let input = nested_database();
    let forge = DataForge::open(&input)?;
    let positioned = first_record(&forge)?;
    let root = positioned.root_fields()?;
    let groups = root.get(0).ok_or("parent array property")?;
    assert!(
        groups.fields()?.is_none(),
        "only a structure has positioned fields"
    );
    assert!(groups.element(2).is_none(), "unadmitted parent ordinal");
    let group = groups.element(1).ok_or("selected second structure")?;
    assert_eq!(
        group.array_header_source(),
        groups.array_header_source(),
        "parent header retained on selected element"
    );
    assert!(
        group.field_name_sources().is_none(),
        "an element has no independent declared name"
    );
    assert!(
        group.element(0).is_none(),
        "a struct cannot be used as an array slot"
    );
    let group_type = group.value_sources();
    assert!(
        group_type.complete(),
        "the declared class and actual definition were observed"
    );
    assert!(
        group_type
            .sources()
            .contains(&RecordJsonSource::StructName { index: 0 }),
        "actual element structure definition"
    );
    assert!(
        !group_type.sources().iter().any(|source| matches!(
            source,
            RecordJsonSource::Bytes {
                kind: RecordJsonBytes::ArrayElement,
                ..
            }
        )),
        "the whole struct body does not determine its type"
    );
    check_nested_values(&input, &group)?;
    Ok(())
}

fn check_nested_values(input: &[u8], group: &PositionedValue<'_, '_>) -> TestResult {
    let fields = group.fields()?.ok_or("second instance fields")?;
    let weights = fields.get(0).ok_or("nested array")?;
    let (_, raw) = bytes_of(
        input,
        &weights.value_sources(),
        RecordJsonBytes::ArrayHeader,
    )?;
    assert_eq!(
        raw,
        [2, 0, 0, 0, 1, 0, 0, 0],
        "the second instance selects two values at pool index one"
    );
    assert_ne!(
        weights.array_header_source(),
        group.array_header_source(),
        "inner and outer headers differ"
    );
    let value = weights.element(1).ok_or("nested second pool slot")?;
    assert_eq!(
        value.array_header_source(),
        weights.array_header_source(),
        "actual inner header"
    );
    assert_eq!(value.value(), &Value::Int32(33), "actual nested element");
    let value_sources = value.value_sources();
    assert!(
        value_sources.complete(),
        "the actual element and count/start words were retained"
    );
    let (_, raw) = bytes_of(input, &value_sources, RecordJsonBytes::ArrayElement)?;
    assert_eq!(raw, 33_i32.to_le_bytes(), "the exact selected pool element");
    assert!(
        weights.element(2).is_none(),
        "count bounds element selection"
    );
    let name = fields.get(1).ok_or("empty text field")?.value_sources();
    let (_, raw) = bytes_of(input, &name, RecordJsonBytes::Text)?;
    assert_eq!(
        raw,
        &[0],
        "the authored empty string is evidenced by its NUL"
    );
    assert!(name.complete(), "empty text has original physical evidence");
    Ok(())
}

#[test]
fn positioned_failed_scalar_retains_raw_word_with_incomplete_value() -> TestResult {
    let input = DataForgeBuilder::new()
        .structure(TestStruct::new("Check").field("valid", TestType::Bool))
        .record(
            TestRecord::new(1, "Check", "Check.One", "check.xml")
                .value("valid", TestValue::Raw(vec![2])),
        )
        .build();
    let forge = DataForge::open(&input)?;
    let positioned = first_record(&forge)?;
    let fields = positioned.root_fields()?;
    let site = fields.get(0).ok_or("bool site")?;
    assert!(
        matches!(site.value(), Value::Error(_)),
        "ordinary decode keeps this error"
    );
    assert!(
        site.storage_sources().complete(),
        "the stored invalid word is fully observed"
    );
    let value = site.value_sources();
    assert!(
        !value.complete(),
        "the failed decode remains partial downstream"
    );
    let (_, raw) = bytes_of(&input, &value, RecordJsonBytes::Slot)?;
    assert_eq!(raw, &[2], "actual invalid byte");
    Ok(())
}

fn first_record<'source>(
    forge: &'source DataForge<'source>,
) -> TestResult<PositionedRecord<'source>> {
    let record = forge.record(0).ok_or("fixture Record")?;
    Ok(TreeBuilder::new().build_positioned(&record)?)
}

fn nested_database() -> Vec<u8> {
    let group = |weights: Vec<i32>, name| {
        TestValue::Struct(
            TestInstance::new("Group")
                .value(
                    "weights",
                    TestValue::Array(weights.into_iter().map(TestValue::Int32).collect()),
                )
                .value("name", TestValue::text(name)),
        )
    };
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Group")
                .array("weights", TestType::Int32)
                .field("name", TestType::String),
        )
        .structure(TestStruct::new("Owner").array("groups", TestType::Class("Group".to_owned())))
        .record(TestRecord::new(1, "Owner", "Owner.One", "owner.xml").value(
            "groups",
            TestValue::Array(vec![group(vec![11], "first"), group(vec![22, 33], "")]),
        ))
        .build()
}

fn bytes_of<'input>(
    input: &'input [u8],
    observations: &PositionedSources,
    expected: RecordJsonBytes,
) -> TestResult<(usize, &'input [u8])> {
    let (start, end) = observations
        .sources()
        .iter()
        .find_map(|source| {
            if let RecordJsonSource::Bytes { start, end, kind } = *source
                && kind == expected
            {
                Some((start, end))
            } else {
                None
            }
        })
        .ok_or("requested original byte range")?;
    let start = usize::try_from(start)?;
    let end = usize::try_from(end)?;
    let bytes = input
        .get(start..end)
        .ok_or("byte range within original input")?;
    Ok((start, bytes))
}
