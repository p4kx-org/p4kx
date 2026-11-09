// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType, TestValue};

use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_values as values;

#[derive(Clone, Copy)]
pub(crate) enum Case {
    TwoOwned,
    Empty,
    NullChild,
    NullRoot,
}

pub(crate) fn database(storage: TestType, case: Case) -> DataForgeBuilder {
    let display = match case {
        Case::TwoOwned => values::sequence(vec![
            values::owned(schema::BASE),
            values::owned(schema::BASE),
        ]),
        Case::Empty => values::sequence(Vec::new()),
        Case::NullChild => values::sequence(vec![TestValue::Null, values::owned(schema::BASE)]),
        Case::NullRoot => TestValue::Null,
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(schema::property())
        .structure(
            TestStruct::new(schema::OVERRIDE)
                .field("propertyName", TestType::Locale)
                .field(
                    "condition",
                    TestType::StrongPointer(schema::CONDITION.to_owned()),
                ),
        )
        .structure(TestStruct::new(schema::CONDITION))
        .structure(TestStruct::new(schema::BASE))
        .structure(TestStruct::new(schema::NON_REF).parent(schema::BASE))
        .structure(
            TestStruct::new(schema::SEQUENCE)
                .parent(schema::NON_REF)
                .array("transformations", storage),
        )
        .record(values::record(values::FIRST, "Primary", display))
}

pub(crate) const fn node_count(case: Case) -> Option<usize> {
    match case {
        Case::TwoOwned => Some(3),
        Case::Empty => Some(1),
        Case::NullChild => Some(2),
        Case::NullRoot => None,
    }
}
