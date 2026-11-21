// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::alloc::Layout;
use std::io;

use p4kx_dataforge::{
    DataForge, DataType, Field, FieldStorage, Record, RecordTree, Value, ValueError,
};
use p4kx_testkit::TestValue;

use crate::record_field_storage_source::{self as source, Case, TestResult};

#[test]
fn original_field_storage_distinguishes_reference_and_weak_scalar_and_array_decoding() -> TestResult
{
    report_layout()?;
    for array in [false, true] {
        for case in [Case::Reference, Case::Weak] {
            let bytes = source::database(case, array);
            let forge = DataForge::open(&bytes)?;
            source::original(&forge, case, array)?;
            let record = forge.record(0).ok_or("actual field owner")?;
            let tree = record.tree()?;
            assert!(
                tree.is_from(&record),
                "actual tree owner precedes storage capture"
            );
            let field = tree.fields().first().ok_or("original physical field")?;
            let receipt = field.storage();
            assert_eq!(
                receipt.data_type(),
                case.data_type(),
                "the original declaration survives decoding"
            );
            assert_eq!(
                receipt.is_array(),
                array,
                "the original array declaration survives"
            );
            assert_eq!(
                receipt.target(),
                0,
                "the actual source target word is retained"
            );
        }
    }
    Ok(())
}

#[test]
fn field_storage_keeps_old_value_equality_wire_and_foreign_owner_rejection() -> TestResult {
    for array in [false, true] {
        let authored = source::database(Case::Reference, array);
        let pointer = source::database(Case::Weak, array);
        let authored = DataForge::open(&authored)?;
        let pointer = DataForge::open(&pointer)?;
        source::original(&authored, Case::Reference, array)?;
        source::original(&pointer, Case::Weak, array)?;
        let owner = authored.record(0).ok_or("actual Reference owner")?;
        let foreign = pointer.record(0).ok_or("actual WeakPointer owner")?;
        let first = owner.tree()?;
        let second = foreign.tree()?;
        equality_and_owner(&first, &second, &owner, &foreign, array)?;
    }
    Ok(())
}

#[test]
fn field_storage_retains_original_pointer_words_and_dangling_guid_errors() -> TestResult {
    for case in [Case::Reference, Case::Weak] {
        let value = match case {
            Case::Reference => TestValue::Reference(source::MISSING),
            Case::Weak => TestValue::Raw(
                [
                    source::INVALID_STRUCT.to_le_bytes(),
                    source::INVALID_INSTANCE.to_le_bytes(),
                ]
                .concat(),
            ),
        };
        let bytes = source::builder(case, false, value).build();
        let forge = DataForge::open(&bytes)?;
        source::original(&forge, case, false)?;
        let record = forge.record(0).ok_or("original error owner")?;
        let tree = record.tree()?;
        let field = tree.fields().first().ok_or("original failed field")?;
        let expected = match case {
            Case::Reference => ValueError::DanglingReference {
                guid: "00000000-0000-0000-0000-000000005813".parse()?,
            },
            Case::Weak => ValueError::InvalidPointer {
                struct_index: source::INVALID_STRUCT,
                instance_index: source::INVALID_INSTANCE,
            },
        };
        assert_eq!(
            field.value(),
            &Value::Error(expected),
            "the genuine decoder error stays unchanged"
        );
        assert_eq!(
            field.storage().data_type(),
            case.data_type(),
            "error values retain their actual storage kind"
        );
    }
    Ok(())
}

fn equality_and_owner(
    first: &RecordTree<'_>,
    second: &RecordTree<'_>,
    owner: &Record<'_>,
    foreign: &Record<'_>,
    array: bool,
) -> TestResult {
    source::equal_links(first, second, array)?;
    let left = first.fields().first().ok_or("actual Reference field")?;
    let right = second.fields().first().ok_or("actual WeakPointer field")?;
    assert_ne!(
        left.storage(),
        right.storage(),
        "actual storage receipts are distinct"
    );
    assert_eq!(left, right, "Field equality remains exactly name and value");
    assert_eq!(
        first, second,
        "RecordTree equality still compares its old visible values"
    );
    assert!(
        !first.is_from(foreign),
        "value equality never authorizes another original owner"
    );
    assert!(
        first.clone().is_from(owner),
        "a genuine whole-tree clone retains its owner"
    );
    assert_eq!(
        source::rendering(first)?,
        source::rendering(second)?,
        "receipt metadata never changes canonical JSON/XML"
    );
    Ok(())
}

fn report_layout() -> io::Result<()> {
    use std::io::Write as _;
    writeln!(
        io::stderr().lock(),
        "FIELD_STORAGE_LAYOUT arch={} os={} field={}/{} storage={}/{} value={}/{} datatype={}/{}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        Layout::new::<Field<'_>>().size(),
        Layout::new::<Field<'_>>().align(),
        Layout::new::<FieldStorage>().size(),
        Layout::new::<FieldStorage>().align(),
        Layout::new::<Value<'_>>().size(),
        Layout::new::<Value<'_>>().align(),
        Layout::new::<DataType>().size(),
        Layout::new::<DataType>().align(),
    )
}
