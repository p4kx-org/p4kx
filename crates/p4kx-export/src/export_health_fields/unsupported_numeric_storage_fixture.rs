// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::catalog_number_capture::FloatToken;

#[derive(Clone, Copy)]
pub(super) enum Case {
    Double,
    Array,
    Owner,
}

pub(super) struct Expected {
    pub(super) owner: &'static str,
    pub(super) field: &'static str,
    pub(super) path: &'static str,
    pub(super) ordinal: u64,
    pub(super) data_type: DataType,
    pub(super) is_array: bool,
    pub(super) token: FloatToken,
}

impl Case {
    pub(super) const ALL: [Self; 3] = [Self::Double, Self::Array, Self::Owner];

    pub(super) const fn expected(self) -> Expected {
        match self {
            Self::Double => Expected {
                owner: "GramsPerCubicCentimeter",
                field: "gramsPerCubicCentimeter",
                path: "densityType.gramsPerCubicCentimeter",
                ordinal: 1,
                data_type: DataType::Double,
                is_array: false,
                token: FloatToken::Double(f64::NAN.to_bits()),
            },
            Self::Array => Expected {
                owner: "GramsPerCubicCentimeter",
                field: "gramsPerCubicCentimeter",
                path: "densityType.gramsPerCubicCentimeter[0]",
                ordinal: 2,
                data_type: DataType::Single,
                is_array: true,
                token: FloatToken::Single(f32::NAN.to_bits()),
            },
            Self::Owner => Expected {
                owner: "FixtureMolarMass",
                field: "molarMass",
                path: "densityType.molarMass",
                ordinal: 1,
                data_type: DataType::Single,
                is_array: false,
                token: FloatToken::Single(f32::NAN.to_bits()),
            },
        }
    }

    pub(super) fn bytes(self) -> Vec<u8> {
        let expected = self.expected();
        let (owner, value) = match self {
            Self::Double => (
                TestStruct::new("GramsPerCubicCentimeter")
                    .field("gramsPerCubicCentimeter", TestType::Double),
                TestValue::Double(f64::NAN),
            ),
            Self::Array => (
                TestStruct::new("GramsPerCubicCentimeter")
                    .array("gramsPerCubicCentimeter", TestType::Single),
                TestValue::Array(vec![TestValue::Single(f32::NAN)]),
            ),
            Self::Owner => (
                TestStruct::new("FixtureMolarMass").field("molarMass", TestType::Single),
                TestValue::Single(f32::NAN),
            ),
        };
        DataForgeBuilder::new()
            .version(8)
            .structure(TestStruct::new("ResourceType").field(
                "densityType",
                TestType::StrongPointer(expected.owner.to_owned()),
            ))
            .structure(owner)
            .record(
                TestRecord::new(
                    1,
                    "ResourceType",
                    "ResourceType.Fixture",
                    "libs/foundry/records/resourcetypedatabase/fixture.xml",
                )
                .value(
                    "densityType",
                    TestValue::Struct(
                        TestInstance::new(expected.owner).value(expected.field, value),
                    ),
                ),
            )
            .build()
    }
}

pub(super) const fn token(value: &Value<'_>) -> Option<FloatToken> {
    match value {
        Value::Single(value) => Some(FloatToken::Single(value.to_bits())),
        Value::Double(value) => Some(FloatToken::Double(value.to_bits())),
        _ => None,
    }
}
