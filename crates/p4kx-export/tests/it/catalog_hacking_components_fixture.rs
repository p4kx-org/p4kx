// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestStruct, TestType, TestValue};

use crate::catalog_tags_fixture as tags;

use super::super::item;

pub(super) const OWNER: u128 = 0x7801;
pub(super) const CHARGES: &str = "HackingChipParams";
pub(super) const MODIFIERS: &str = "RemovableChipParams";
pub(super) const MODIFIER: &str = "RemovableChipValue";
pub(super) const WRONG_TARGET: &str = "FixtureUnrelatedModifier";
pub(super) const BAD_OFFSET: u32 = u32::MAX - 1;
pub(super) const NAMES: [&str; 3] = ["ErrorChance", "ErrorChance", "FixtureUnknownModifier"];
pub(super) const SINGLES: [f32; 3] = [0.0, -0.0, 1.25];

#[derive(Clone, Copy)]
pub(super) enum Case {
    Valid,
    WrongStorage,
    WrongEmptyTarget,
    Unreadable,
}

pub(super) fn database(case: Case) -> DataForgeBuilder {
    let storage = match case {
        Case::WrongStorage => TestType::StrongPointer(MODIFIER.to_owned()),
        Case::WrongEmptyTarget => TestType::Class(WRONG_TARGET.to_owned()),
        Case::Valid | Case::Unreadable => TestType::Class(MODIFIER.to_owned()),
    };
    schema(storage)
        .record(
            item(OWNER, "fixture_hacking_chip", "Misc", slots(case)).value(
                "tags",
                tags::references(&[match case {
                    Case::Valid | Case::WrongStorage | Case::WrongEmptyTarget => tags::ROOT,
                    Case::Unreadable => tags::MISSING,
                }]),
            ),
        )
        .record(
            item(
                OWNER + 1,
                "fixture_hacking_chip_name_only",
                "Misc",
                Vec::new(),
            )
            .value("tags", tags::references(&[])),
        )
        .record(tags::tag(
            tags::ROOT,
            "FixtureAccess",
            tags::references(&[]),
        ))
        .record(tags::roots(tags::references(&[tags::ROOT])))
}

fn schema(storage: TestType) -> DataForgeBuilder {
    tags::schema()
        .enumeration("FixtureChipItemType", ["Misc"])
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("tags", TestType::ReferenceTo("Tag".to_owned()))
                .array(
                    "Components",
                    TestType::StrongPointer("DataForgeComponentParams".to_owned()),
                ),
        )
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(TestStruct::new(WRONG_TARGET))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", TestType::Class("SItemDefinition".to_owned())),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::Enum("FixtureChipItemType".to_owned())),
        )
        .structure(
            TestStruct::new(CHARGES)
                .parent("DataForgeComponentParams")
                .field("maxCharges", TestType::Int32),
        )
        .structure(
            TestStruct::new(MODIFIERS)
                .parent("DataForgeComponentParams")
                .array("values", storage),
        )
        .structure(
            TestStruct::new(MODIFIER)
                .field("name", TestType::String)
                .field("value", TestType::Single),
        )
}

fn slots(case: Case) -> Vec<TestValue> {
    let mut modifiers: Vec<_> = NAMES
        .into_iter()
        .zip(SINGLES)
        .map(|(name, number)| modifier(TestValue::text(name), number))
        .collect();
    if matches!(case, Case::WrongEmptyTarget) {
        modifiers.clear();
    }
    if matches!(case, Case::Unreadable) {
        modifiers.insert(
            0,
            modifier(TestValue::Raw(BAD_OFFSET.to_le_bytes().to_vec()), 2.0),
        );
    }
    vec![
        TestValue::Struct(TestInstance::new(CHARGES).value("maxCharges", TestValue::Int32(0))),
        removable(modifiers),
        removable(Vec::new()),
    ]
}

fn removable(values: Vec<TestValue>) -> TestValue {
    TestValue::Struct(TestInstance::new(MODIFIERS).value("values", TestValue::Array(values)))
}

fn modifier(name: TestValue, number: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(MODIFIER)
            .value("name", name)
            .value("value", TestValue::Single(number)),
    )
}
