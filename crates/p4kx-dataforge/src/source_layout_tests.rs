// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

use crate::query::QuerySchema;
use crate::source_layout_allocation;
use crate::{DataForge, DataForgeError, SNIFF_LEN};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const STRUCT_ROW_BYTES: usize = 16;
const PROPERTY_ROW_BYTES: usize = 12;

#[path = "../tests/it/layout_field_u64_fixture.rs"]
mod wide;

#[test]
fn physical_layouts_keep_one_copy_of_each_original_property() -> TestResult {
    let mut builder = DataForgeBuilder::new();
    for index in 0..160 {
        let mut row = TestStruct::new(format!("S{index}"))
            .field(format!("f{index}"), TestType::UInt8)
            .declared_size(index + 1);
        if index != 0 {
            row = row.parent(format!("S{}", index - 1));
        }
        builder = builder.structure(row);
    }
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    for structure in forge.structs() {
        assert_eq!(
            structure.properties().count(),
            1,
            "one actual own declaration per row"
        );
    }
    let forge = borrow_for_read(&forge);
    let index = forge.source_layouts()?;
    assert!(
        std::ptr::eq(index, forge.source_layouts()?),
        "successful metadata are shared"
    );
    assert_eq!(
        index.physical_fields().len(),
        160,
        "no retained inherited prefixes"
    );
    let layout = index.layout(forge, 159)?;
    assert_eq!(
        layout.field_count(),
        160,
        "all original inherited occurrences"
    );
    for (position, field) in layout.cursor()?.enumerate() {
        let field = field?;
        assert_eq!(
            field.name,
            format!("f{position}"),
            "root-first original name"
        );
        assert_eq!(
            (field.position, field.start, field.end),
            (position as u64, position as u64, position as u64 + 1),
            "exact original coordinates"
        );
    }
    Ok(())
}

fn borrow_for_read<'read, 'bytes: 'read>(
    forge: &'read DataForge<'bytes>,
) -> &'read DataForge<'read> {
    forge
}

#[test]
fn source_layout_index_rejects_a_second_forge_over_identical_bytes() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("A").field("v", TestType::UInt8))
        .build();
    let first = DataForge::open(&bytes)?;
    let second = DataForge::open(&bytes)?;
    let index = first.source_layouts()?;
    assert!(
        index.layout(&first, 0).is_ok(),
        "the actual source borrow is accepted"
    );
    assert!(
        matches!(
            index.layout(&second, 0),
            Err(DataForgeError::LayoutInvariant)
        ),
        "equal bytes cannot substitute another owning DataForge"
    );
    assert!(
        index.layout(&first, 0).is_ok(),
        "foreign rejection changes no metadata"
    );
    Ok(())
}

#[test]
fn unused_bad_properties_do_not_poison_an_independent_valid_range() -> TestResult {
    let mut bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Good").field("good", TestType::UInt8))
        .structure(TestStruct::new("Bad").field("bad", TestType::UInt8))
        .build();
    let datatype = SNIFF_LEN + 2 * STRUCT_ROW_BYTES + PROPERTY_ROW_BYTES + 6;
    patch(&mut bytes, datatype, &u16::MAX.to_le_bytes())?;
    let forge = DataForge::open(&bytes)?;
    let index = forge.source_layouts()?;
    assert_eq!(
        index.layout(&forge, 0)?.field_count(),
        1,
        "the good original range remains complete"
    );
    assert!(
        matches!(
            index.layout(&forge, 1),
            Err(DataForgeError::UnknownDataType { code: u16::MAX })
        ),
        "the actual malformed declaration retains its diagnosis"
    );
    let schema = QuerySchema::of(&forge)?;
    let text = "select ";
    let completion = schema
        .complete(text, text.len())
        .ok_or("the actual good declaration")?;
    let names: Vec<_> = completion
        .candidates()
        .iter()
        .map(crate::query::Candidate::name)
        .collect();
    assert_eq!(
        names,
        ["good"],
        "an invalid layout never supplies a field suggestion"
    );
    Ok(())
}

#[test]
fn root_size_error_precedes_child_payload_type_and_cycles_precede_both() -> TestResult {
    let mut bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Base")
                .field("first", TestType::UInt8)
                .declared_size(2),
        )
        .structure(
            TestStruct::new("Child")
                .parent("Base")
                .field("next", TestType::UInt8)
                .declared_size(3),
        )
        .build();
    let datatype = SNIFF_LEN + 2 * STRUCT_ROW_BYTES + PROPERTY_ROW_BYTES + 6;
    patch(&mut bytes, datatype, &u16::MAX.to_le_bytes())?;
    let forge = DataForge::open(&bytes)?;
    assert!(
        matches!(
            forge.source_layouts()?.layout(&forge, 1),
            Err(DataForgeError::StructLayout {
                struct_index: 0,
                declared: 2,
                fields: 1
            })
        ),
        "the earliest original ancestor is checked first"
    );
    drop(forge);
    patch(&mut bytes, SNIFF_LEN + 4, &1_u32.to_le_bytes())?;
    let cyclic = DataForge::open(&bytes)?;
    assert!(
        matches!(
            cyclic.source_layouts()?.layout(&cyclic, 1),
            Err(DataForgeError::StructCycle { struct_index: 1 })
        ),
        "the requested original cycle identity"
    );
    Ok(())
}

#[test]
fn completion_keeps_first_inherited_declaration_and_global_original_struct_order() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Derived")
                .parent("Base")
                .field("dup", TestType::UInt32)
                .declared_size(7),
        )
        .structure(TestStruct::new("Other").field("dup", TestType::Bool))
        .structure(
            TestStruct::new("Base")
                .field("dup", TestType::UInt8)
                .field("Dup", TestType::Int16),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let schema = QuerySchema::of(&forge)?;
    for text in ["select ", "from Derived select "] {
        let completion = schema
            .complete(text, text.len())
            .ok_or("inherited original fields")?;
        let names: Vec<_> = completion
            .candidates()
            .iter()
            .map(crate::query::Candidate::name)
            .collect();
        assert_eq!(
            names,
            ["Dup", "dup"],
            "case variants stay distinct in name order"
        );
        let duplicate = completion
            .candidates()
            .iter()
            .find(|field| field.name() == "dup")
            .ok_or("the original duplicated name")?;
        assert_eq!(
            duplicate.detail(),
            Some("UInt8"),
            "the first inherited occurrence of the earliest valid struct wins"
        );
    }
    drop(forge);
    assert!(
        schema.complete("from Derived select ", 20).is_some(),
        "owned schema survives its source"
    );
    Ok(())
}

#[test]
fn impossible_metadata_reserve_is_an_operation_error() {
    let mut bytes = Vec::<u8>::new();
    let failure = source_layout_allocation::reserve(&mut bytes, usize::MAX, "test metadata");
    assert!(
        matches!(
            failure,
            Err(DataForgeError::LayoutAllocationRefused {
                part: "test metadata",
                ..
            })
        ),
        "a capacity refusal does not invent malformed source metadata"
    );
    assert!(bytes.is_empty(), "no successful prefix is retained");
}

fn patch(bytes: &mut [u8], at: usize, value: &[u8]) -> TestResult {
    bytes
        .get_mut(at..at + value.len())
        .ok_or("the actual encoded field")?
        .copy_from_slice(value);
    Ok(())
}

#[test]
fn billions_of_zero_size_original_fields_remain_metadata_only() -> TestResult {
    let bytes = wide::database(SNIFF_LEN)?;
    let forge = DataForge::open(&bytes)?;
    assert_eq!(
        forge.struct_count(),
        wide::SEGMENTS as usize + 1,
        "the actual parent rows"
    );
    assert_eq!(
        forge.property_count(),
        usize::from(wide::OWN),
        "one shared physical property range"
    );
    let record = forge.record(0).ok_or("the actual zero-byte owner")?;
    let row = record.struct_type()?;
    assert_eq!(
        (row.index(), row.size()),
        (wide::SEGMENTS, 0),
        "the original Record selects the last zero-size row"
    );
    assert_eq!(
        row.properties().count(),
        usize::from(wide::OWN),
        "the last original own count is still u16"
    );
    let index = forge.source_layouts()?;
    assert_eq!(
        index.physical_fields().len(),
        usize::from(wide::OWN),
        "logical repetition does not multiply metadata storage"
    );
    let layout = index.layout(&forge, wide::SEGMENTS)?;
    let expected = u64::from(wide::SEGMENTS) * u64::from(wide::OWN);
    assert!(
        expected > u64::from(u32::MAX),
        "the original inherited field count has no u32 bound"
    );
    assert_eq!(
        layout.field_count(),
        expected,
        "the complete checked count fits u64"
    );
    let mut cursor = layout.cursor()?;
    for position in 0..2_u64 {
        let field = cursor
            .next()
            .ok_or("one bounded original metadata occurrence")??;
        assert_eq!(
            (field.position, field.start, field.end),
            (position, 0, 0),
            "zero bytes do not remove real field occurrences"
        );
    }
    Ok(())
}
