// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

#[derive(Clone, Copy)]
pub(super) enum Product {
    Missing,
    Present,
    Blacklisted,
}

pub(super) fn database(product: Product, broken: bool, alias: bool) -> DataForgeBuilder {
    let owner = TestStruct::new("CraftingBlueprintRecord").field("blueprint", pointer("Blueprint"));
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(if broken {
            owner.declared_size(1)
        } else {
            owner
        })
        .structure(
            TestStruct::new("Blueprint")
                .field("blueprintName", TestType::Locale)
                .field("category", TestType::Reference)
                .field("processSpecificData", pointer("Process"))
                .array("tiers", TestType::Class("Tier".to_owned())),
        )
        .structure(TestStruct::new("Tier"))
        .structure(TestStruct::new("Process"))
        .structure(TestStruct::new("CraftProcess").parent("Process").field(
            "entityClass",
            TestType::ReferenceTo("EntityClassDefinition".to_owned()),
        ))
        .structure(
            TestStruct::new("GenericCraftingProcess_Dismantle")
                .parent("Process")
                .field("efficiency", TestType::Single),
        )
        .structure(
            TestStruct::new("CraftingGlobalParams")
                .field(
                    "defaultBlueprintSelection",
                    TestType::Class("Defaults".to_owned()),
                )
                .array("dismantleBlacklistEntityClasses", TestType::Reference),
        )
        .structure(TestStruct::new("Defaults").array("blueprintRecords", TestType::Reference))
        .structure(
            TestStruct::new("EntityClassDefinition")
                .declared_size(1)
                .field("value", TestType::UInt64),
        )
        .structure(
            TestStruct::new("Foreign")
                .declared_size(1)
                .field("value", TestType::UInt64),
        );
    let builder = records(builder, product);
    if alias {
        builder.record(owner_record(product, "Alias"))
    } else {
        builder
    }
}

fn records(builder: DataForgeBuilder, product: Product) -> DataForgeBuilder {
    let blacklist = if matches!(product, Product::Blacklisted) {
        vec![TestValue::Reference(4)]
    } else {
        Vec::new()
    };
    builder
        .record(owner_record(product, "Primary"))
        .record(
            record(
                2,
                "CraftingGlobalParams",
                "crafting/globalparams/primary.xml",
            )
            .value(
                "defaultBlueprintSelection",
                TestValue::Struct(TestInstance::new("Defaults").value(
                    "blueprintRecords",
                    TestValue::Array(vec![TestValue::Reference(1)]),
                )),
            )
            .value(
                "dismantleBlacklistEntityClasses",
                TestValue::Array(blacklist),
            ),
        )
        .record(
            record(
                3,
                "CraftingBlueprintRecord",
                "crafting/blueprints/dismantle/primary.xml",
            )
            .value(
                "blueprint",
                TestValue::Struct(TestInstance::new("Blueprint").value(
                    "processSpecificData",
                    TestValue::Struct(TestInstance::new("GenericCraftingProcess_Dismantle")),
                )),
            ),
        )
        .record(record(4, "EntityClassDefinition", "entities/primary.xml"))
        .record(record(5, "Foreign", "unrelated/primary.xml"))
}

fn owner_record(product: Product, name: &str) -> TestRecord {
    let process = if matches!(product, Product::Missing) {
        TestValue::Null
    } else {
        TestValue::Struct(
            TestInstance::new("CraftProcess").value("entityClass", TestValue::Reference(4)),
        )
    };
    TestRecord::new(
        1,
        "CraftingBlueprintRecord",
        format!("CraftingBlueprintRecord.{name}"),
        format!(
            "libs/foundry/records/crafting/blueprints/crafting/{}.xml",
            name.to_ascii_lowercase()
        ),
    )
    .value(
        "blueprint",
        TestValue::Struct(TestInstance::new("Blueprint").value("processSpecificData", process)),
    )
}

fn record(guid: u128, kind: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        kind,
        format!("{kind}.Primary"),
        format!("libs/foundry/records/{path}"),
    )
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
