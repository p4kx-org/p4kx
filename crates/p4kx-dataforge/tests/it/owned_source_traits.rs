// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::owned_source_controls as controls;
use crate::owned_source_fixture::{self as fixture, TestResult};
use p4kx_dataforge::{DataForge, RecordTree, Value};
use std::fmt;

#[test]
fn complete_original_owned_values_survive_clone_eq_drop_and_flat_debug() -> TestResult {
    let bytes = fixture::database(2, 257);
    let forge = DataForge::open(&bytes)?;
    let record = controls::original(&forge, 2, 257)?;
    let original = record.tree()?;
    controls::complete(&original, 2, 257)?;
    controls::tail(&original, &record, 2, 257)?;
    let cloned = original.clone();
    assert_eq!(
        original, cloned,
        "Clone preserves every existing semantic value"
    );
    assert!(
        cloned.is_from(&record),
        "the whole clone retains its actual original owner"
    );
    shared_location(&original, &cloned)?;
    drop(original);
    controls::complete(&cloned, 2, 257)?;
    controls::tail(&cloned, &record, 2, 257)?;
    debug_source(&cloned);
    Ok(())
}

fn shared_location(original: &RecordTree<'_>, cloned: &RecordTree<'_>) -> TestResult {
    let Some(Value::Reference(original)) = original.field("tail") else {
        return Err("the actual original weak target".into());
    };
    let Some(Value::Reference(cloned)) = cloned.field("tail") else {
        return Err("the same original target in the whole clone".into());
    };
    assert!(
        std::ptr::eq(
            original.location().ok_or("original target location")?,
            cloned.location().ok_or("cloned target location")?,
        ),
        "cloning shares the same original cached Location rather than duplicating its path"
    );
    Ok(())
}

fn debug_source(tree: &RecordTree<'_>) {
    let compact = format!("{:?}", tree.root());
    let pretty = format!("{:#?}", tree.root());
    for dump in [&compact, &pretty] {
        assert_eq!(
            dump.matches("BeginStruct(").count(),
            514,
            "every original Node begins once"
        );
        assert_eq!(
            dump.matches("EndStruct").count(),
            514,
            "every original Node ends once"
        );
        assert_eq!(
            dump.matches("Null").count(),
            2,
            "both original terminal nulls survive"
        );
    }
    for mark in 1..=514 {
        assert!(
            compact.contains(&format!("UInt32({mark})")),
            "the original scalar marker {mark} survives complete Debug output"
        );
    }
    assert!(
        compact.contains("Element(0)"),
        "the first actual root slot exists"
    );
    assert!(
        compact.contains("Element(1)"),
        "the second actual root slot exists"
    );
    assert_eq!(
        compact.matches("BeginArray(2)").count(),
        1,
        "the original two-element root array begins once"
    );
    assert_eq!(
        compact.matches("EndArray").count(),
        1,
        "the same original array ends once"
    );
}

struct Expected<'value> {
    name: &'static str,
    value: &'value dyn fmt::Debug,
}

impl fmt::Debug for Expected<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple(self.name).field(self.value).finish()
    }
}

fn flags(value: &Value<'_>, expected: &dyn fmt::Debug) {
    assert_eq!(
        format!("{value:?}"),
        format!("{expected:?}"),
        "ordinary standard Debug"
    );
    assert_eq!(
        format!("{value:#?}"),
        format!("{expected:#?}"),
        "alternate standard Debug"
    );
    assert_eq!(
        format!("{value:x?}"),
        format!("{expected:x?}"),
        "lower hexadecimal Debug flags"
    );
    assert_eq!(
        format!("{value:X?}"),
        format!("{expected:X?}"),
        "upper hexadecimal Debug flags"
    );
    assert_eq!(
        format!("{value:#x?}"),
        format!("{expected:#x?}"),
        "alternate hexadecimal flags"
    );
    assert_eq!(
        format!("{value:+020.3?}"),
        format!("{expected:+020.3?}"),
        "sign width zero padding and precision"
    );
}

#[test]
fn scalar_debug_modes_and_explicit_array_boundaries_are_complete() -> TestResult {
    macro_rules! scalar {
        ($kind:ident, $raw:expr) => {{
            let raw = $raw;
            flags(
                &Value::$kind(raw),
                &Expected {
                    name: stringify!($kind),
                    value: &raw,
                },
            );
        }};
    }
    scalar!(Bool, true);
    scalar!(Int8, i8::MIN);
    scalar!(Int16, i16::MIN);
    scalar!(Int32, i32::MIN);
    scalar!(Int64, i64::MIN);
    scalar!(UInt8, u8::MAX);
    scalar!(UInt16, u16::MAX);
    scalar!(UInt32, u32::MAX);
    scalar!(UInt64, u64::MAX);
    scalar!(Single, f32::from_bits(0x7fc0_1234));
    scalar!(Double, f64::NEG_INFINITY);
    scalar!(String, "Gr\u{fc}\u{df}e");
    scalar!(Locale, "@original");
    scalar!(Enum, "future_source_label");
    scalar!(Guid, "01234567-89ab-cdef-0123-456789abcdef".parse()?);
    array_slots();
    Ok(())
}

fn array_slots() {
    let value =
        Value::Array(vec![Value::Null, Value::Array(Box::default()), Value::UInt8(9)].into());
    let dump = format!("{value:?}");
    for fragment in [
        "Element(0), Null",
        "Element(1), BeginArray(0), EndArray",
        "Element(2), UInt8(9)",
    ] {
        assert!(
            dump.contains(fragment),
            "the explicit actual SDK slot survives: {fragment}"
        );
    }
    let nan = Value::Single(f32::from_bits(0x7fc0_1234));
    assert!(
        matches!(nan.clone(), Value::Single(copy) if copy.to_bits() == 0x7fc0_1234),
        "Clone preserves the same scalar variant and exact original IEEE payload bits"
    );
    assert_ne!(
        nan,
        nan.clone(),
        "NaN keeps its existing PartialEq semantics"
    );
}

#[test]
fn scalar_equality_keeps_primitive_zero_nan_infinity_and_adjacent_values() {
    for (left, right, expected) in [
        (-0.0, 0.0, true),
        (f32::INFINITY, f32::INFINITY, true),
        (1.25, 1.25, true),
        (1.0, 1.0 + f32::EPSILON, false),
        (f32::INFINITY, f32::NEG_INFINITY, false),
        (f32::NAN, f32::NAN, false),
    ] {
        assert_eq!(
            Value::Single(left).eq(&Value::Single(right)),
            expected,
            "Single keeps primitive PartialEq without bit equality or tolerance"
        );
    }
    for (left, right, expected) in [
        (-0.0, 0.0, true),
        (f64::INFINITY, f64::INFINITY, true),
        (1.25, 1.25, true),
        (1.0, 1.0 + f64::EPSILON, false),
        (f64::INFINITY, f64::NEG_INFINITY, false),
        (f64::NAN, f64::NAN, false),
    ] {
        assert_eq!(
            Value::Double(left).eq(&Value::Double(right)),
            expected,
            "Double keeps primitive PartialEq without bit equality or tolerance"
        );
    }
    assert_ne!(
        Value::Single(0.0),
        Value::Double(0.0),
        "storage variants remain distinct"
    );
}
