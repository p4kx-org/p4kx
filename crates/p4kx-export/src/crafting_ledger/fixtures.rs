// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const RECORD: u128 = 0x6600_0000_0000_4000_8000_0000_0000_0001;

pub(super) fn blueprint(depth: usize, leftover: bool) -> Vec<u8> {
    let mut cost = TestInstance::new("CraftingCost_Item").value("quantity", TestValue::Int32(1));
    for _ in 0..depth {
        cost = TestInstance::new("CraftingCost_Select")
            .value("options", TestValue::Array(vec![TestValue::Struct(cost)]));
    }
    let tier = TestInstance::new("CraftingBlueprintTier").value(
        "recipe",
        TestValue::Struct(TestInstance::new("Recipe").value(
            "costs",
            TestValue::Struct(
                TestInstance::new("Costs").value("mandatoryCost", TestValue::Struct(cost)),
            ),
        )),
    );
    let name = if leftover {
        "synthetic_test"
    } else {
        "synthetic_blueprint"
    };
    blueprint_schema()
        .record(
            TestRecord::new(
                RECORD,
                "CraftingBlueprintRecord",
                format!("CraftingBlueprintRecord.{name}"),
                format!("libs/foundry/records/crafting/blueprints/crafting/{name}.xml"),
            )
            .value(
                "blueprint",
                TestValue::Struct(
                    TestInstance::new("CraftingBlueprint")
                        .value("tiers", TestValue::Array(vec![TestValue::Struct(tier)])),
                ),
            ),
        )
        .build()
}

fn blueprint_schema() -> DataForgeBuilder {
    [
        TestStruct::new("CraftingBlueprintRecord")
            .field("blueprint", TestType::Class("CraftingBlueprint".to_owned())),
        TestStruct::new("CraftingBlueprint")
            .array("tiers", TestType::Class("CraftingBlueprintTier".to_owned())),
        TestStruct::new("CraftingBlueprintTier")
            .field("recipe", TestType::Class("Recipe".to_owned())),
        TestStruct::new("Recipe").field("costs", TestType::Class("Costs".to_owned())),
        TestStruct::new("Costs").field(
            "mandatoryCost",
            TestType::StrongPointer("CraftingCost".to_owned()),
        ),
        TestStruct::new("CraftingCost"),
        TestStruct::new("CraftingCost_Select")
            .parent("CraftingCost")
            .array(
                "options",
                TestType::StrongPointer("CraftingCost".to_owned()),
            ),
        TestStruct::new("CraftingCost_Item")
            .parent("CraftingCost")
            .field("quantity", TestType::Int32),
    ]
    .into_iter()
    .fold(DataForgeBuilder::new(), DataForgeBuilder::structure)
}

pub(crate) fn broken(type_name: &str, name: &str, path: &str) -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new(type_name)
                .field("value", TestType::Int32)
                .declared_size(6),
        )
        .record(TestRecord::new(RECORD, type_name, name, path))
        .build()
}

pub(crate) fn fields() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Empty"))
        .structure(
            TestStruct::new("CraftingGlobalParams")
                .field("bad", TestType::Bool)
                .field("nullable", TestType::StrongPointer("Empty".to_owned()))
                .field("link", TestType::Reference)
                .array("empty", TestType::Int32),
        )
        .record(
            TestRecord::new(
                RECORD,
                "CraftingGlobalParams",
                "CraftingGlobalParams.Synthetic",
                "libs/foundry/records/crafting/globalparams/synthetic.xml",
            )
            .value("bad", TestValue::Raw(vec![2]))
            .value("link", TestValue::Reference(RECORD + 1)),
        )
        .build()
}

pub(super) fn no_dismantle() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("CraftingBlueprintRecord")
                .field("blueprint", TestType::Class("Blueprint".to_owned())),
        )
        .structure(TestStruct::new("Blueprint").field(
            "processSpecificData",
            TestType::StrongPointer("Process".to_owned()),
        ))
        .structure(TestStruct::new("Process"))
        .structure(TestStruct::new("CraftingProcess_Creation").parent("Process"))
        .record(
            TestRecord::new(
                RECORD,
                "CraftingBlueprintRecord",
                "CraftingBlueprintRecord.Synthetic",
                "libs/foundry/records/crafting/blueprints/dismantle/synthetic.xml",
            )
            .value(
                "blueprint",
                TestValue::Struct(TestInstance::new("Blueprint").value(
                    "processSpecificData",
                    TestValue::Struct(TestInstance::new("CraftingProcess_Creation")),
                )),
            ),
        )
        .build()
}
