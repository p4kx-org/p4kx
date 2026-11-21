// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const OWNER: &str = "ContractScopeGlobalParams";
const ITEM: &str = "ContractScope_Item";
const DERIVED: &str = "FixtureDerivedScope";
const NEIGHBOR: &str = "FixtureUnrelatedScope";
pub(super) const ICON: &str = "Fixture/Icon";
pub(super) const LABEL: &str = "@shared";
const SLOTS: [&str; 7] = [
    "location",
    "cluster",
    "sector",
    "planetaryBody",
    "planetarySystem",
    "solarSystem",
    "universe",
];

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Complete,
    Unreadable,
}

pub(super) fn database(layout: Layout) -> DataForgeBuilder {
    let definition = SLOTS
        .into_iter()
        .fold(TestStruct::new(OWNER), |root, slot| {
            root.field(slot, TestType::StrongPointer(ITEM.to_owned()))
        });
    let record = SLOTS.into_iter().fold(
        TestRecord::new(
            0x1086,
            OWNER,
            "ContractScopeGlobalParams.Fixture",
            "fixture/scope.xml",
        ),
        |row, slot| row.value(slot, scope_item(slot, layout)),
    );
    DataForgeBuilder::new()
        .version(8)
        .structure(definition)
        .structure(
            TestStruct::new(ITEM)
                .field("displayName", TestType::Locale)
                .field("icon", TestType::String),
        )
        .structure(
            TestStruct::new(DERIVED)
                .parent(ITEM)
                .field("fixtureExtra", TestType::Bool),
        )
        .structure(TestStruct::new(NEIGHBOR).field("icon", TestType::String))
        .record(record)
        .record(
            TestRecord::new(
                0x1087,
                NEIGHBOR,
                "FixtureUnrelatedScope.Neighbor",
                "fixture/neighbor.xml",
            )
            .value("icon", invalid_string()),
        )
}

fn scope_item(slot: &str, layout: Layout) -> TestValue {
    let sector = slot == "sector";
    let concrete = if sector { DERIVED } else { ITEM };
    let icon = if sector && matches!(layout, Layout::Unreadable) {
        invalid_string()
    } else {
        TestValue::text(ICON)
    };
    let item = TestInstance::new(concrete)
        .value("displayName", TestValue::text(LABEL))
        .value("icon", icon);
    TestValue::Struct(if sector {
        item.value("fixtureExtra", TestValue::Bool(true))
    } else {
        item
    })
}

fn invalid_string() -> TestValue {
    TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec())
}
