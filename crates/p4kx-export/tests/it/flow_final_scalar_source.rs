// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{DataType, Guid, RecordTree, StructValue, Value, ValueError};

use crate::flow_final_fixture::{self as fixture, Case};
use crate::flow_final_source;
use crate::health_origin_fixture::TestResult;

pub(crate) const fn declarations() -> [(&'static str, DataType, &'static str); 19] {
    [
        ("flag", DataType::Bool, "bool"),
        ("i8", DataType::Int8, "int8"),
        ("i16", DataType::Int16, "int16"),
        ("i32", DataType::Int32, "int32"),
        ("i64", DataType::Int64, "int64"),
        ("u8", DataType::UInt8, "uint8"),
        ("u16", DataType::UInt16, "uint16"),
        ("u32", DataType::UInt32, "uint32"),
        ("u64", DataType::UInt64, "uint64"),
        ("text", DataType::String, "string"),
        ("single", DataType::Single, "single"),
        ("double", DataType::Double, "double"),
        ("locale", DataType::Locale, "locale"),
        ("guid", DataType::Guid, "guid"),
        ("enum", DataType::Enum, "enum"),
        ("class", DataType::Class, "class"),
        ("strong", DataType::StrongPointer, "strongPointer"),
        ("weak", DataType::WeakPointer, "weakPointer"),
        ("reference", DataType::Reference, "reference"),
    ]
}

pub(crate) fn check(tree: &RecordTree<'_>, case: Case) -> TestResult {
    if absent_or_null(tree, case)? {
        return Ok(());
    }
    let root = condition(
        tree.field("missionFlow").ok_or("actual first root flow")?,
        case,
    )?;
    if let Some(root) = root {
        if matches!(case, Case::Holes) {
            holes(root)?;
        } else {
            let name = if matches!(case, Case::Unknown) {
                fixture::UNKNOWN
            } else {
                fixture::KNOWN
            };
            all_types(root, name, matches!(case, Case::KnownNan))?;
        }
    }
    duplicate(tree, case)?;
    let Some(Value::Single(value)) = tree.field("outsideNumber") else {
        return Err("actual outside numeric datum".into());
    };
    assert_eq!(
        value.to_bits(),
        if matches!(case, Case::OutsideNan) {
            fixture::NAN
        } else {
            0
        },
        "uncovered outside value is a distinct original occurrence"
    );
    Ok(())
}

fn condition<'tree, 'source>(
    value: &'tree Value<'source>,
    case: Case,
) -> TestResult<Option<&'tree StructValue<'source>>> {
    let flow = flow_final_source::structure(value)?;
    assert_eq!(
        flow.type_name(),
        "MissionFlow",
        "original concrete flow type"
    );
    let Some(Value::Array(triggers)) = flow.field("triggers") else {
        return Err("actual original trigger array".into());
    };
    if matches!(case, Case::Empty) {
        assert!(triggers.is_empty(), "genuine present empty trigger array");
        return Ok(None);
    }
    assert_eq!(triggers.len(), 1, "one actual original trigger");
    let trigger = flow_final_source::structure(triggers.first().ok_or("actual trigger")?)?;
    let field = trigger
        .fields()
        .first()
        .ok_or("actual condition declaration")?;
    assert_eq!(
        field.name(),
        "condition",
        "original selected condition field"
    );
    assert_eq!(
        field.storage().data_type(),
        DataType::StrongPointer,
        "genuine original strong condition slot"
    );
    Ok(Some(flow_final_source::structure(field.value())?))
}

fn all_types(root: &StructValue<'_>, name: &str, nan: bool) -> TestResult {
    assert_eq!(
        root.type_name(),
        name,
        "actual condition type before interpretation"
    );
    check_declarations(root);
    values(root);
    let Some(Value::Single(single)) = root.field("single") else {
        return Err("actual Single".into());
    };
    let Some(Value::Double(double)) = root.field("double") else {
        return Err("actual Double".into());
    };
    assert_eq!(
        single.to_bits(),
        if nan {
            fixture::NAN
        } else {
            (-0.0_f32).to_bits()
        },
        "exact Single width and IEEE bits"
    );
    assert_eq!(
        double.to_bits(),
        (-0.0_f64).to_bits(),
        "exact Double negative-zero bits"
    );
    pointers(root)
}

fn values(root: &StructValue<'_>) {
    for (name, expected) in [
        ("flag", Value::Bool(false)),
        ("i8", Value::Int8(i8::MIN)),
        ("i16", Value::Int16(i16::MIN)),
        ("i32", Value::Int32(i32::MIN)),
        ("i64", Value::Int64(i64::MIN)),
        ("u8", Value::UInt8(u8::MAX)),
        ("u16", Value::UInt16(u16::MAX)),
        ("u32", Value::UInt32(u32::MAX)),
        ("u64", Value::UInt64(u64::MAX)),
        ("text", Value::String(fixture::TEXT)),
        ("locale", Value::Locale(fixture::LOCALE)),
        ("enum", Value::Enum(fixture::ENUM)),
        ("guid", Value::Guid(Guid::default())),
    ] {
        assert_eq!(
            root.field(name),
            Some(&expected),
            "exact original source value {name}"
        );
    }
}

fn pointers(root: &StructValue<'_>) -> TestResult {
    for (name, marker) in [("class", -9), ("strong", -10)] {
        let value = flow_final_source::structure(root.field(name).ok_or("actual owned source")?)?;
        assert_eq!(
            value.type_name(),
            "ExtraOwned",
            "actual inline/strong owner"
        );
        assert_eq!(
            value.field("marker"),
            Some(&Value::Int32(marker)),
            "real nested owned facts"
        );
    }
    let Some(Value::Reference(weak)) = root.field("weak") else {
        return Err("actual resolved weak slot".into());
    };
    assert_eq!(
        weak.type_name(),
        "ExtraOwned",
        "actual selected instance type"
    );
    assert_eq!(
        weak.original_target().instance_positions(),
        Some(&[1][..]),
        "real instance receipt points outside the Flow subtree"
    );
    let Some(Value::Reference(reference)) = root.field("reference") else {
        return Err("actual stored GUID Reference".into());
    };
    assert_eq!(
        reference.guid().to_string(),
        fixture::TARGET,
        "actual stored GUID selects the second physical Record"
    );
    assert_eq!(
        reference.original_target().record_index(),
        1,
        "actual selected target position"
    );
    Ok(())
}

fn holes(root: &StructValue<'_>) -> TestResult {
    assert_eq!(
        root.type_name(),
        "MissionFlowCondition_AND",
        "actual conjunction owner"
    );
    let field = root.fields().first().ok_or("actual child declaration")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::StrongPointer,
        "genuine pointer-pool array"
    );
    assert!(field.storage().is_array(), "actual array declaration");
    let Value::Array(values) = field.value() else {
        return Err("actual ordered slots".into());
    };
    assert_eq!(values.len(), 4, "all original equal/null/error slots exist");
    let first = values.first().ok_or("actual first equal leaf")?;
    let last = values.last().ok_or("actual second equal leaf")?;
    assert_eq!(
        first, last,
        "two equal contents are distinct original owned occurrences"
    );
    assert!(!std::ptr::eq(first, last), "different actual array slots");
    all_types(flow_final_source::structure(first)?, fixture::KNOWN, false)?;
    all_types(flow_final_source::structure(last)?, fixture::KNOWN, false)?;
    assert_eq!(
        values.get(1),
        Some(&Value::Null),
        "actual null is not omitted"
    );
    assert_eq!(
        values.get(2),
        Some(&Value::Error(ValueError::InvalidPointer {
            struct_index: 77,
            instance_index: 99
        })),
        "exact original malformed pointer words"
    );
    Ok(())
}
fn absent_or_null(tree: &RecordTree<'_>, case: Case) -> TestResult<bool> {
    if matches!(case, Case::Absent) {
        assert!(
            tree.fields()
                .iter()
                .all(|field| field.name() != "missionFlow"),
            "genuine missing source declaration"
        );
        assert_eq!(tree.fields().len(), 3, "absence is not a null placeholder");
        return Ok(true);
    }
    if matches!(case, Case::Null) {
        let field = tree.fields().get(2).ok_or("actual declared null field")?;
        assert_eq!(
            field.storage().data_type(),
            DataType::StrongPointer,
            "actual nullable original storage"
        );
        assert_eq!(
            field.value(),
            &Value::Null,
            "genuine null pointer occurrence"
        );
        return Ok(true);
    }
    Ok(false)
}

fn duplicate(tree: &RecordTree<'_>, case: Case) -> TestResult {
    if matches!(case, Case::Duplicate) {
        assert_eq!(
            tree.fields()
                .iter()
                .filter(|field| field.name() == "missionFlow")
                .count(),
            2,
            "two real equal-name declarations survive original decode"
        );
        let second = tree
            .fields()
            .get(4)
            .ok_or("second original same-name declaration")?;
        all_types(
            condition(second.value(), Case::Unknown)?.ok_or("real shadowed condition")?,
            fixture::UNKNOWN,
            false,
        )?;
    }
    Ok(())
}

fn check_declarations(root: &StructValue<'_>) {
    assert_eq!(root.fields().len(), 19, "all19 genuine source fields exist");
    let mut seen = HashSet::new();
    for (field, (name, data_type, _)) in root.fields().iter().zip(declarations()) {
        assert_eq!(field.name(), name, "actual original declaration order/name");
        assert_eq!(
            field.storage().data_type(),
            data_type,
            "original decoder storage"
        );
        assert!(!field.storage().is_array(), "original scalar declaration");
        seen.insert(data_type);
    }
    assert_eq!(
        seen.len(),
        19,
        "every distinct current original storage kind"
    );
}
