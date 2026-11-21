// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::commodity_v3_fixture::TestResult;
use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_values as values;
use p4kx_extraction::{DataForge, DataType, RecordTree, StructValue, Value as Raw};
use p4kx_testkit::DataForgeBuilder;

pub(crate) fn with_tree(
    source: &DataForgeBuilder,
    index: u32,
    check: impl FnOnce(&RecordTree<'_>) -> TestResult,
) -> TestResult {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge
        .record(index)
        .ok_or("original physical property")?
        .tree()?;
    assert_eq!(
        tree.record_index(),
        index,
        "the tree retains the original physical owner"
    );
    check(&tree)
}

pub(crate) fn structure<'a, 'f>(value: Option<&'a Raw<'f>>) -> TestResult<&'a StructValue<'f>> {
    let Some(Raw::Struct(value)) = value else {
        return Err("original owned struct".into());
    };
    Ok(value)
}

pub(crate) fn array<'a, 'f>(value: Option<&'a Raw<'f>>) -> TestResult<&'a [Raw<'f>]> {
    let Some(Raw::Array(value)) = value else {
        return Err("original array".into());
    };
    Ok(value)
}

pub(crate) fn basic(source: &DataForgeBuilder) -> TestResult {
    with_tree(source, 0, |tree| {
        assert_eq!(
            tree.guid().to_string(),
            crate::commodity_v3_fixture::guid(values::FIRST),
            "original nominal identity"
        );
        assert_eq!(
            tree.type_name(),
            schema::PROPERTY,
            "original root source type"
        );
        assert_eq!(
            tree.metadata().file_name(),
            values::FIRST_FILE,
            "original raw filename"
        );
        assert_eq!(
            tree.root().field("propertyName"),
            Some(&Raw::Locale("@resource_name")),
            "the property name is an actual Locale"
        );
        assert_eq!(
            tree.root().field("unitFormat"),
            Some(&Raw::Locale("@own_type")),
            "the unit format is an independent actual Locale"
        );
        let display = structure(tree.root().field("displayTransformation"))?;
        assert_eq!(
            display.type_name(),
            schema::SCALE,
            "original concrete pointer target"
        );
        let Some(Raw::Single(number)) = display.field("scale") else {
            return Err("original Single scale".into());
        };
        assert_eq!(
            number.to_bits(),
            1.25_f32.to_bits(),
            "actual original IEEE bits"
        );
        assert!(
            array(tree.root().field("nameOverrides"))?.is_empty(),
            "the original class array is declared and empty"
        );
        Ok(())
    })
}

pub(crate) fn declarations(source: &DataForgeBuilder) -> TestResult {
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let expected = [
        (schema::PROPERTY, None, 4),
        (schema::OVERRIDE, None, 2),
        (schema::BASE, None, 0),
        (schema::NON_REF, Some(schema::BASE), 0),
        (schema::NEGATED, Some(schema::NON_REF), 0),
        (schema::PERCENT, Some(schema::NON_REF), 0),
        (schema::FACTOR, Some(schema::NON_REF), 0),
        (schema::SCALE, Some(schema::NON_REF), 1),
        (schema::SEQUENCE, Some(schema::NON_REF), 1),
        (schema::CONDITION, None, 0),
        (schema::CONDITION_NON_REF, Some(schema::CONDITION), 0),
        (schema::ITEM_TYPE, Some(schema::CONDITION_NON_REF), 2),
    ];
    assert_eq!(
        forge.structs().count(),
        12,
        "the fixture implements the complete observed closure"
    );
    for (name, parent, count) in expected {
        let declaration = forge
            .structs()
            .find(|item| item.name() == Ok(name))
            .ok_or("actual original declaration")?;
        assert_eq!(
            declaration
                .parent()
                .map(|parent| parent.name())
                .transpose()?,
            parent,
            "the original parent link of {name}"
        );
        assert_eq!(
            declaration.properties().count(),
            count,
            "own members of {name}"
        );
    }
    assert_members(&forge)
}

fn assert_members(forge: &DataForge<'_>) -> TestResult {
    for expected in MEMBERS {
        let owner = forge
            .structs()
            .find(|item| item.name() == Ok(expected.declaring_type))
            .ok_or("declaring owner")?;
        let field = owner
            .properties()
            .nth(expected.ordinal)
            .ok_or("original declared ordinal")?;
        assert_eq!(
            field.name()?,
            expected.name,
            "the original declaring-local member ordinal"
        );
        assert_eq!(
            (field.data_type()?, field.is_array()),
            (expected.kind, expected.is_array),
            "the original member's scalar/array declaration"
        );
        let actual = if expected.kind == DataType::Enum {
            field
                .target_enum()
                .map(|target| target.name())
                .transpose()?
        } else {
            field
                .target_struct()
                .map(|target| target.name())
                .transpose()?
        };
        assert_eq!(
            actual, expected.target,
            "the exact original declared target"
        );
    }
    Ok(())
}

struct Member {
    declaring_type: &'static str,
    ordinal: usize,
    name: &'static str,
    kind: DataType,
    is_array: bool,
    target: Option<&'static str>,
}

const MEMBERS: &[Member] = &[
    Member {
        declaring_type: schema::PROPERTY,
        ordinal: 0,
        name: "propertyName",
        kind: DataType::Locale,
        is_array: false,
        target: None,
    },
    Member {
        declaring_type: schema::PROPERTY,
        ordinal: 1,
        name: "displayTransformation",
        kind: DataType::StrongPointer,
        is_array: false,
        target: Some(schema::BASE),
    },
    Member {
        declaring_type: schema::PROPERTY,
        ordinal: 2,
        name: "unitFormat",
        kind: DataType::Locale,
        is_array: false,
        target: None,
    },
    Member {
        declaring_type: schema::PROPERTY,
        ordinal: 3,
        name: "nameOverrides",
        kind: DataType::Class,
        is_array: true,
        target: Some(schema::OVERRIDE),
    },
    Member {
        declaring_type: schema::OVERRIDE,
        ordinal: 0,
        name: "propertyName",
        kind: DataType::Locale,
        is_array: false,
        target: None,
    },
    Member {
        declaring_type: schema::OVERRIDE,
        ordinal: 1,
        name: "condition",
        kind: DataType::StrongPointer,
        is_array: false,
        target: Some(schema::CONDITION),
    },
    Member {
        declaring_type: schema::SCALE,
        ordinal: 0,
        name: "scale",
        kind: DataType::Single,
        is_array: false,
        target: None,
    },
    Member {
        declaring_type: schema::SEQUENCE,
        ordinal: 0,
        name: "transformations",
        kind: DataType::StrongPointer,
        is_array: true,
        target: Some(schema::BASE),
    },
    Member {
        declaring_type: schema::ITEM_TYPE,
        ordinal: 0,
        name: "matchItemTypes",
        kind: DataType::Enum,
        is_array: true,
        target: Some("EItemType"),
    },
    Member {
        declaring_type: schema::ITEM_TYPE,
        ordinal: 1,
        name: "matchSubTypes",
        kind: DataType::Enum,
        is_array: true,
        target: Some("EItemSubType"),
    },
];
