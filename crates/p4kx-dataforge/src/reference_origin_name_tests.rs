// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ptr;

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::{DataForge, DataForgeError, RecordText, Value};

type TestResult = Result<(), Box<dyn Error>>;
const CALLER: u128 = 0xe980_0000_0000_4000_8000_0000_0000_0001;
const TARGET: u128 = 0xe980_0000_0000_4000_8000_0000_0000_0002;
const NAME: &str = "ReceiptTarget.LongOriginalHeaderName";

fn database(target: TestRecord) -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ReceiptCaller").field("target", TestType::Reference))
        .structure(TestStruct::new("ReceiptTarget"))
        .record(
            TestRecord::new(
                CALLER,
                "ReceiptCaller",
                "ReceiptCaller.Source",
                "caller.xml",
            )
            .value("target", TestValue::Reference(TARGET)),
        )
        .record(target)
        .record(TestRecord::new(
            TARGET,
            "ReceiptTarget",
            "ReceiptTarget.EqualGuidAlias",
            "alias.xml",
        ))
        .build()
}

fn target(name: &str) -> TestRecord {
    TestRecord::new(TARGET, "ReceiptTarget", name, "selected.xml")
}

#[test]
fn original_target_name_borrows_the_selected_header_despite_equal_guid_aliases() -> TestResult {
    let bytes = database(target(NAME));
    let copied_bytes = bytes.clone();
    let forge = DataForge::open(&bytes)?;
    let copied_forge = DataForge::open(&copied_bytes)?;
    let caller = forge.record(0).ok_or("actual source caller")?;
    let selected = forge.record(1).ok_or("actual selected target")?;
    let alias = forge.record(2).ok_or("actual equal-GUID alternative")?;
    assert_eq!(
        selected.guid(),
        alias.guid(),
        "genuine nominal alias negative control"
    );
    assert_ne!(
        selected.name()?,
        alias.name()?,
        "genuine header values distinguish targets"
    );
    let tree = caller.tree()?;
    let Value::Reference(reference) = tree.root().field("target").ok_or("actual reference")? else {
        return Err("the original decoder must return the real Reference".into());
    };
    let receipt = reference.original_target();
    assert!(
        receipt.shares_source_with(&caller),
        "genuine original owner admission"
    );
    assert_eq!(
        receipt.record_index(),
        1,
        "frozen first physical target selection"
    );
    assert_eq!(
        receipt.record_path(),
        "selected",
        "actual selected target, not its alias"
    );
    let name = receipt.record_name()?;
    assert_eq!(
        name,
        selected.stored_name()?,
        "exact original metadata state and text"
    );
    let RecordText::Present(name) = name else {
        return Err("original present name".into());
    };
    assert!(
        ptr::eq::<str>(name, selected.name()?),
        "the getter borrows original header bytes"
    );
    check_copied_input(reference, name, &copied_forge)
}

fn check_copied_input(
    reference: &crate::Reference<'_>,
    name: &str,
    copied_forge: &DataForge<'_>,
) -> TestResult {
    let receipt = reference.original_target();
    let copied = copied_forge.record(0).ok_or("equal copied caller")?;
    let copied_tree = copied.tree()?;
    let Value::Reference(copied_reference) = copied_tree
        .root()
        .field("target")
        .ok_or("actual copied reference")?
    else {
        return Err("copied original Reference".into());
    };
    assert_eq!(
        reference,
        copied_reference.as_ref(),
        "equal descriptors are storage equality only"
    );
    assert!(
        !receipt.shares_source_with(&copied),
        "equal copied inputs remain foreign"
    );
    assert!(
        !ptr::eq::<str>(
            name,
            copied_reference
                .original_target()
                .record_name()?
                .fold(|| "", || "", |name| name)
        ),
        "equal text does not merge genuine byte owners"
    );
    Ok(())
}

#[test]
fn original_target_name_preserves_stored_null_and_present_empty() -> TestResult {
    for (row, expected) in [
        (target(NAME).without_name(), RecordText::Null),
        (target(""), RecordText::Present("")),
    ] {
        let bytes = database(row);
        let forge = DataForge::open(&bytes)?;
        let record = forge.record(0).ok_or("genuine caller")?;
        let tree = record.tree()?;
        let Value::Reference(reference) =
            tree.root().field("target").ok_or("original reference")?
        else {
            return Err("the original decoder retains the actual target".into());
        };
        assert_eq!(
            tree.error_count(),
            0,
            "the genuine target selection is readable"
        );
        assert_eq!(
            reference.record_index(),
            1,
            "the selected physical header is fixed"
        );
        assert_eq!(
            reference.original_target().record_name()?,
            expected,
            "null and empty remain different original declarations"
        );
    }
    Ok(())
}

#[test]
fn original_target_name_returns_the_actual_invalid_string_error() -> TestResult {
    let mut bytes = database(target(NAME));
    let needle = format!("{NAME}\0").into_bytes();
    let positions = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, found)| (found == needle).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1, "one actual stored target header string");
    let position = *positions.first().ok_or("unique actual header bytes")?;
    *bytes.get_mut(position).ok_or("actual first header byte")? = u8::MAX;
    let forge = DataForge::open(&bytes)?;
    let caller = forge.record(0).ok_or("actual caller")?;
    let selected = forge.record(1).ok_or("actual selected target")?;
    let expected = selected.stored_name();
    assert!(
        matches!(
            expected,
            Err(DataForgeError::InvalidString { table: "blob", .. })
        ),
        "the real stored header became unreadable"
    );
    let tree = caller.tree()?;
    assert_eq!(
        tree.error_count(),
        0,
        "the actual reference remains readable independently"
    );
    let Value::Reference(reference) = tree.root().field("target").ok_or("actual reference")? else {
        return Err("the original reference still resolves".into());
    };
    assert_eq!(
        reference.record_index(),
        1,
        "the same actual target was admitted"
    );
    assert_eq!(
        reference.original_target().record_name(),
        expected,
        "the getter preserves every original error parameter instead of inventing absence"
    );
    Ok(())
}
