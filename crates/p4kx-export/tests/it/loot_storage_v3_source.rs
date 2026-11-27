// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashSet};

use p4kx_extraction::{DataType, Guid, RecordTree, SourceEncryption, Value};

use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::loot_storage_v3_fixture as fixture;

pub(crate) struct Original {
    positions: BTreeMap<String, OriginalField>,
}

struct OriginalField {
    ordinal: u64,
    target: u16,
}

impl Original {
    pub(crate) fn ordinal(&self, field: &str) -> TestResult<u64> {
        self.positions
            .get(field)
            .map(|value| value.ordinal)
            .ok_or_else(|| "independently visited source field".into())
    }
    pub(crate) fn target(&self, field: &str) -> TestResult<u16> {
        self.positions
            .get(field)
            .map(|value| value.target)
            .ok_or_else(|| "actual original declaration target".into())
    }
}

pub(crate) const fn declarations() -> [(&'static str, DataType, &'static str); 22] {
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
        ("unitFormat", DataType::String, "string"),
        ("empty", DataType::String, "string"),
        ("signedMax", DataType::Int64, "int64"),
    ]
}

pub(crate) fn check(input: &Fixture, kind: Input) -> TestResult<Original> {
    let expected = match kind {
        Input::Plain => SourceEncryption::Unencrypted,
        Input::Encrypted => SourceEncryption::Encrypted,
        Input::Standalone => SourceEncryption::Unknown,
    };
    assert_eq!(
        input.entry.origin().encryption(),
        expected,
        "actual original input encryption precedes output markers"
    );
    let forge = input.entry.open()?;
    assert_eq!(
        forge.records().len(),
        2,
        "both genuine source Records exist"
    );
    let record = forge.record(0).ok_or("actual scalar owner")?;
    let target = forge.record(1).ok_or("actual scalar reference target")?;
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "actual original tree/Record pairing");
    assert_eq!(record.index(), 0, "actual physical source position");
    assert_eq!(
        record.guid().to_string(),
        fixture::OWNER,
        "actual source GUID"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "all19 original storage types are genuinely readable"
    );
    assert_eq!(
        target.tree()?.error_count(),
        0,
        "target source is independently readable"
    );
    check_declarations(&tree);
    scalars(&tree)?;
    class(&tree)?;
    pointers(&tree)?;
    positions(&tree)
}

fn check_declarations(tree: &RecordTree<'_>) {
    assert_eq!(
        tree.fields().len(),
        22,
        "all original declarations including extra strings exist"
    );
    let mut kinds = HashSet::new();
    for (field, (name, data_type, _)) in tree.fields().iter().zip(declarations()) {
        assert_eq!(field.name(), name, "actual original declaration order");
        assert_eq!(
            field.storage().data_type(),
            data_type,
            "actual decoder storage type"
        );
        assert!(
            !field.storage().is_array(),
            "all19 scalar-storage source witnesses"
        );
        kinds.insert(data_type);
    }
    assert_eq!(
        kinds.len(),
        19,
        "every supported distinct original DataType"
    );
}

fn positions(tree: &RecordTree<'_>) -> TestResult<Original> {
    let mut next = 0_u64;
    let mut positions = BTreeMap::new();
    for field in tree.fields() {
        assert!(
            positions
                .insert(
                    field.name().to_owned(),
                    OriginalField {
                        ordinal: next,
                        target: field.storage().target()
                    }
                )
                .is_none(),
            "these original names are unique"
        );
        next += 1;
        if let Value::Struct(value) = field.value() {
            next += u64::try_from(value.fields().len())?;
        }
    }
    assert_eq!(
        next, 26,
        "all original scalar/Struct/Reference occurrences counted before JSON"
    );
    Ok(Original { positions })
}

fn scalars(tree: &RecordTree<'_>) -> TestResult {
    let expected = [
        ("flag", Value::Bool(false)),
        ("i8", Value::Int8(i8::MIN)),
        ("i16", Value::Int16(i16::MIN)),
        ("i32", Value::Int32(i32::MIN)),
        ("i64", Value::Int64(i64::MIN)),
        ("signedMax", Value::Int64(i64::MAX)),
        ("u8", Value::UInt8(u8::MAX)),
        ("u16", Value::UInt16(u16::MAX)),
        ("u32", Value::UInt32(u32::MAX)),
        ("u64", Value::UInt64(u64::MAX)),
        ("text", Value::String(fixture::TEXT)),
        ("empty", Value::String("")),
        ("locale", Value::Locale(fixture::LOCALE)),
        ("enum", Value::Enum(fixture::ENUM)),
        ("unitFormat", Value::String(fixture::TEMPLATE)),
        ("guid", Value::Guid(Guid::default())),
    ];
    for (name, value) in expected {
        assert_eq!(
            tree.field(name),
            Some(&value),
            "exact actual original {name}"
        );
    }
    let Some(Value::Single(single)) = tree.field("single") else {
        return Err("actual Single".into());
    };
    let Some(Value::Double(double)) = tree.field("double") else {
        return Err("actual Double".into());
    };
    assert_eq!(
        single.to_bits(),
        0x8000_0000,
        "actual Single negative zero bits"
    );
    assert_eq!(
        double.to_bits(),
        0x8000_0000_0000_0000,
        "actual Double negative zero bits"
    );
    Ok(())
}

fn class(tree: &RecordTree<'_>) -> TestResult {
    let Some(Value::Struct(class)) = tree.field("class") else {
        return Err("actual inline Class".into());
    };
    assert_eq!(
        class.type_name(),
        "CraftingGameplayPropertyDef",
        "actual inline owner controls text domains"
    );
    assert_eq!(class.fields().len(), 2, "exact original inline child count");
    assert_eq!(
        class.field("propertyName"),
        Some(&Value::String("temperature")),
        "actual public fact"
    );
    assert_eq!(
        class.field("unitFormat"),
        Some(&Value::Locale("%.2f RPM")),
        "actual public format template"
    );
    Ok(())
}

fn pointers(tree: &RecordTree<'_>) -> TestResult {
    let Some(Value::Struct(strong)) = tree.field("strong") else {
        return Err("actual owned StrongPointer".into());
    };
    assert_eq!(
        strong.type_name(),
        "PoolFilterInstance",
        "actual designer-text owner"
    );
    assert_eq!(
        strong.fields().len(),
        2,
        "exact original strong child count"
    );
    assert_eq!(
        strong.field("name"),
        Some(&Value::String("@designer.literal")),
        "actual literal designer name"
    );
    assert_eq!(
        strong.field("mode"),
        Some(&Value::String("Cumulative")),
        "actual literal designer mode"
    );
    let Some(Value::Reference(weak)) = tree.field("weak") else {
        return Err("actual resolved WeakPointer".into());
    };
    assert_eq!(
        weak.type_name(),
        "PoolFilterInstance",
        "actual weak target type"
    );
    assert_eq!(
        weak.original_target().instance_positions(),
        Some(&[16][..]),
        "genuine original strong-field instance receipt"
    );
    let Some(Value::Reference(reference)) = tree.field("reference") else {
        return Err("actual stored Reference".into());
    };
    assert_eq!(
        reference.guid().to_string(),
        fixture::TARGET,
        "actual admitted target GUID"
    );
    Ok(())
}
