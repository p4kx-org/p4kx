// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestStruct, TestType, TestValue, entity_schema,
};

use crate::hackable_device_fixture as device;

pub(super) const CHIP_RECORD: u32 = 0;
pub(super) const DEVICE_RECORD: u32 = 1;

const CHARGE: &str = "HackingChipParams";
const REMOVABLE: &str = "RemovableChipParams";
const MODIFIER: &str = "RemovableChipValue";

pub(super) fn database() -> DataForgeBuilder {
    device::schema(entity_schema(DataForgeBuilder::new().version(8)))
        .structure(component(CHARGE).field("maxCharges", TestType::Int32))
        .structure(component(REMOVABLE).array("values", TestType::Class(MODIFIER.to_owned())))
        .structure(
            TestStruct::new(MODIFIER)
                .field("name", TestType::String)
                .field("value", TestType::Single),
        )
        .record(
            TestEntity::new(
                0x71f1,
                "FixtureHackingChip",
                "libs/foundry/records/entities/scitem/ships/fixture_hacking_chip.xml",
            )
            .attach_type("Misc")
            .component(charge())
            .component(removable())
            .record(),
        )
        .record(
            TestEntity::new(
                0x71f2,
                "FixtureHackingDevice",
                "libs/foundry/records/entities/fixture/hacking_device.xml",
            )
            .component(charge())
            .component(device::component())
            .record(),
        )
}

fn component(name: &str) -> TestStruct {
    TestStruct::new(name).parent("DataForgeComponentParams")
}

fn charge() -> TestValue {
    TestValue::Struct(TestInstance::new(CHARGE).value("maxCharges", TestValue::Int32(0)))
}

fn removable() -> TestValue {
    let modifiers = [0.0, -0.0]
        .into_iter()
        .map(|single| {
            TestValue::Struct(
                TestInstance::new(MODIFIER)
                    .value("name", TestValue::text("FixtureModifier"))
                    .value("value", TestValue::Single(single)),
            )
        })
        .collect();
    TestValue::Struct(TestInstance::new(REMOVABLE).value("values", TestValue::Array(modifiers)))
}
