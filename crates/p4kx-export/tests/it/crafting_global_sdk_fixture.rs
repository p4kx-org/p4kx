// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const OWNER: u128 = 0x1166_0000_0000_4000_8000_0000_0000_c001;
pub(super) const BLUEPRINT: u128 = 0x1166_0000_0000_4000_8000_0000_0000_c002;
pub(super) const RESOURCE: u128 = 0x1166_0000_0000_4000_8000_0000_0000_c003;
pub(super) const ITEM: u128 = 0x1166_0000_0000_4000_8000_0000_0000_c004;
pub(super) const DANGLING: u128 = 0x1166_0000_0000_4000_8000_0000_0000_c099;

pub(super) fn database() -> DataForgeBuilder {
    declarations(false)
        .record(owner(
            "Primary",
            vec![
                TestValue::Reference(BLUEPRINT),
                TestValue::Null,
                TestValue::Reference(BLUEPRINT),
                TestValue::Reference(DANGLING),
            ],
        ))
        .record(owner("Alias", vec![TestValue::Reference(BLUEPRINT)]))
        .record(TestRecord::new(
            BLUEPRINT,
            "CraftingBlueprintRecord",
            "Blueprint.Unreadable",
            "libs/foundry/records/crafting/blueprints/crafting/unreadable.xml",
        ))
        .record(TestRecord::new(
            RESOURCE,
            "ResourceType",
            "Resource.Unreadable",
            "libs/foundry/records/resourcetypes/unreadable.xml",
        ))
        .record(TestRecord::new(
            ITEM,
            "EntityClassDefinition",
            "Entity.Unreadable",
            "libs/foundry/records/entities/items/unreadable.xml",
        ))
}

pub(super) fn declarations(broken: bool) -> DataForgeBuilder {
    let global = TestStruct::new("CraftingGlobalParams")
        .field(
            "defaultBlueprintSelection",
            TestType::StrongPointer("Defaults".to_owned()),
        )
        .array("dismantleBlacklistResources", TestType::Reference)
        .array("dismantleBlacklistEntityClasses", TestType::Reference)
        .field("futureThreshold", TestType::Double);
    DataForgeBuilder::new()
        .version(8)
        .structure(if broken {
            global.declared_size(1)
        } else {
            global
        })
        .structure(
            TestStruct::new("Defaults")
                .array("blueprintRecords", TestType::Reference)
                .field("futureRule", TestType::Bool),
        )
        .structure(
            TestStruct::new("CraftingBlueprintRecord")
                .field("bad", TestType::UInt64)
                .declared_size(1),
        )
        .structure(
            TestStruct::new("ResourceType")
                .field("bad", TestType::UInt64)
                .declared_size(1),
        )
        .structure(
            TestStruct::new("EntityClassDefinition")
                .field("bad", TestType::UInt64)
                .declared_size(1),
        )
}

pub(super) fn owner(name: &str, defaults: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        OWNER,
        "CraftingGlobalParams",
        format!("CraftingGlobalParams.{name}"),
        format!(
            "libs/foundry/records/crafting/globalparams/{}.xml",
            name.to_ascii_lowercase()
        ),
    )
    .value(
        "defaultBlueprintSelection",
        TestValue::Struct(
            TestInstance::new("Defaults")
                .value("blueprintRecords", TestValue::Array(defaults))
                .value("futureRule", TestValue::Bool(false)),
        ),
    )
    .value(
        "dismantleBlacklistResources",
        TestValue::Array(vec![
            TestValue::Reference(RESOURCE),
            TestValue::Null,
            TestValue::Reference(RESOURCE),
        ]),
    )
    .value(
        "dismantleBlacklistEntityClasses",
        TestValue::Array(vec![
            TestValue::Reference(ITEM),
            TestValue::Reference(DANGLING),
        ]),
    )
    .value(
        "futureThreshold",
        TestValue::Double(f64::from_bits(0x7ff8_0000_0000_00a1)),
    )
}
