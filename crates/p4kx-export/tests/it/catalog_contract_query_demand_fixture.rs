// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const OWNER: u128 = 0x8100_0000_0000_4000_8000_0000_0000_0001;
const CONTRACT: u128 = 0x8200_0000_0000_4000_8000_0000_0000_0001;
const TEMPLATE: u128 = 0x8300_0000_0000_4000_8000_0000_0000_0001;
const PLACE: u128 = 0x8400_0000_0000_4000_8000_0000_0000_0001;

#[derive(Clone, Copy)]
pub(super) enum Used {
    Neither,
    Template,
    Place,
}

impl Used {
    pub(super) const fn target(self) -> Option<u32> {
        match self {
            Self::Neither => None,
            Self::Template => Some(1),
            Self::Place => Some(2),
        }
    }
}

pub(super) fn database(used: Used) -> Vec<u8> {
    schema()
        .into_iter()
        .fold(
            DataForgeBuilder::new().version(8),
            DataForgeBuilder::structure,
        )
        .record(generator(used))
        .record(
            TestRecord::new(
                TEMPLATE,
                "ContractTemplate",
                "ContractTemplate.Unused",
                "libs/foundry/records/contracts/contracttemplates/unused.xml",
            )
            .with_instance_index(u16::MAX),
        )
        .record(
            TestRecord::new(
                PLACE,
                "StarMapObject",
                "StarMapObject.Unused",
                "libs/foundry/records/starmap/pu/unused.xml",
            )
            .with_instance_index(u16::MAX),
        )
        .build()
}

fn schema() -> [TestStruct; 7] {
    [
        TestStruct::new("ContractGenerator")
            .array("generators", pointer("Handler"))
            .field("unusedTemplate", TestType::Reference)
            .field("unusedPlace", TestType::Reference),
        TestStruct::new("Handler").array("contracts", pointer("Contract")),
        TestStruct::new("Contract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .field("template", TestType::Reference)
            .array("additionalPrerequisites", pointer("ContractPrerequisite")),
        TestStruct::new("ContractPrerequisite"),
        TestStruct::new("ContractPrerequisite_Location")
            .parent("ContractPrerequisite")
            .field("locationAvailable", TestType::Reference),
        TestStruct::new("ContractTemplate"),
        TestStruct::new("StarMapObject").field("name", TestType::Locale),
    ]
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn generator(used: Used) -> TestRecord {
    let mut contract = TestInstance::new("Contract")
        .value("id", TestValue::Guid(CONTRACT))
        .value("debugName", TestValue::text("Selected"));
    if matches!(used, Used::Template) {
        contract = contract.value("template", TestValue::Reference(TEMPLATE));
    }
    if matches!(used, Used::Place) {
        contract = contract.value(
            "additionalPrerequisites",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new("ContractPrerequisite_Location")
                    .value("locationAvailable", TestValue::Reference(PLACE)),
            )]),
        );
    }
    TestRecord::new(
        OWNER,
        "ContractGenerator",
        "ContractGenerator.Carrier",
        "libs/foundry/records/contracts/contractgenerator/carrier.xml",
    )
    .value("unusedTemplate", TestValue::Reference(TEMPLATE))
    .value("unusedPlace", TestValue::Reference(PLACE))
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(TestInstance::new("Handler").value(
            "contracts",
            TestValue::Array(vec![TestValue::Struct(contract)]),
        ))]),
    )
}
