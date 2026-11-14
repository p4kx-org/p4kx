// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestValue};

use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::mission_property_sets_fixture::{generator, parameters};
use crate::mission_values_fixture::{self as values, Owner};
use crate::scoped_fixture::{object, property, schema, scoped};

#[derive(Clone, Copy)]
pub(crate) enum Case {
    UnknownValue,
    UnknownRelative,
    UnreadableSearch,
}

impl Case {
    pub(crate) const fn source_type(self) -> &'static str {
        match self {
            Self::UnknownValue => "MissionPropertyValue_Unknown",
            Self::UnknownRelative => "LocationCondition_UnknownRelative",
            Self::UnreadableSearch => "MissionPropertyValue_AutoSpawns",
        }
    }

    pub(crate) const fn field(self) -> &'static str {
        match self {
            Self::UnknownValue => "property value",
            Self::UnknownRelative => "relativeLocationFilter",
            Self::UnreadableSearch => "searchOptions",
        }
    }
}

pub(crate) fn open(case: Case, input: Input) -> TestResult<Fixture> {
    let source = match case {
        Case::UnknownValue => TestInstance::new("MissionPropertyValue_Unknown"),
        Case::UnknownRelative => scoped([(
            "relativeLocationFilter",
            object(TestInstance::new("LocationCondition_UnknownRelative")),
        )]),
        Case::UnreadableSearch => TestInstance::new("MissionPropertyValue_AutoSpawns").value(
            "searchOptions",
            TestValue::Raw(
                (u32::MAX - 1)
                    .to_le_bytes()
                    .into_iter()
                    .chain(3_u32.to_le_bytes())
                    .collect(),
            ),
        ),
    };
    let properties = vec![property("Nested", source)];
    let builder = match case {
        Case::UnreadableSearch => values::database(properties, Owner::Generator),
        _ => schema(DataForgeBuilder::new().version(8)).record(generator(std::array::from_fn(
            |_| parameters(properties.clone()),
        ))),
    };
    Fixture::open(&builder.build(), input, Vec::new())
}
