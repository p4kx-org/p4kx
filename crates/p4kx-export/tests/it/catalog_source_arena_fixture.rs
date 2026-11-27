// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(crate) const OWNER: u128 = 0xc100_0000_0000_4000_8000_0000_0000_0001;
pub(super) const TARGET: u128 = 0xc100_0000_0000_4000_8000_0000_0000_0002;
pub(super) const DANGLING: u128 = 0xc100_0000_0000_4000_8000_0000_0000_0003;

pub(crate) fn database() -> DataForgeBuilder {
    let source = [
        TestStruct::new("Target").field("marker", TestType::Int32),
        TestStruct::new("Owned").field("marker", TestType::Int32),
        TestStruct::new("RawNumbers")
            .field("signed", TestType::Int64)
            .field("unsigned", TestType::UInt64)
            .field("single", TestType::Single),
        TestStruct::new("CraftingGameplayPropertyDef")
            .field("unitFormat", TestType::String)
            .field("note", TestType::Locale),
        TestStruct::new("ContractGenerator")
            .field("numbers", TestType::Class("RawNumbers".to_owned()))
            .field(
                "texts",
                TestType::Class("CraftingGameplayPropertyDef".to_owned()),
            )
            .array("sequence", TestType::ReferenceTo("Target".to_owned()))
            .field("owned", TestType::StrongPointer("Owned".to_owned()))
            .field("weakOwned", TestType::WeakPointer("Owned".to_owned()))
            .field("weakRecord", TestType::WeakPointer("Target".to_owned()))
            .field("broken", TestType::Bool)
            .array("empty", TestType::Int32)
            .field("nullPointer", TestType::StrongPointer("Owned".to_owned()))
            .field("identity", TestType::Guid)
            .field("enabled", TestType::Bool),
    ]
    .into_iter()
    .fold(
        DataForgeBuilder::new().version(8),
        DataForgeBuilder::structure,
    );
    source
        .record(
            TestRecord::new(
                TARGET,
                "Target",
                "Target.Present",
                "libs/foundry/records/fixture/target.xml",
            )
            .value("marker", TestValue::Int32(9)),
        )
        .record(owner())
}

fn owner() -> TestRecord {
    TestRecord::new(
        OWNER,
        "ContractGenerator",
        "ContractGenerator.SourceArena",
        "libs/foundry/records/contracts/contractgenerator/source_arena.xml",
    )
    .value(
        "numbers",
        TestValue::Struct(
            TestInstance::new("RawNumbers")
                .value("signed", TestValue::Int64(i64::MIN))
                .value("unsigned", TestValue::UInt64(u64::MAX))
                .value("single", TestValue::Single(0.1)),
        ),
    )
    .value(
        "texts",
        TestValue::Struct(
            TestInstance::new("CraftingGameplayPropertyDef")
                .value("unitFormat", TestValue::text("%.2f RPM"))
                .value("note", TestValue::text("@original_note")),
        ),
    )
    .value(
        "sequence",
        TestValue::Array(vec![
            TestValue::Reference(TARGET),
            TestValue::Null,
            TestValue::Reference(DANGLING),
            TestValue::Reference(TARGET),
        ]),
    )
    .value(
        "owned",
        TestValue::Struct(
            TestInstance::new("Owned")
                .named("selected-owned-instance")
                .value("marker", TestValue::Int32(7)),
        ),
    )
    .value("weakOwned", TestValue::pointer("selected-owned-instance"))
    .value("weakRecord", TestValue::pointer("Target.Present"))
    .value("broken", TestValue::Raw(vec![2]))
    .value("empty", TestValue::Array(Vec::new()))
    .value("nullPointer", TestValue::Null)
    .value("identity", TestValue::Guid(OWNER))
    .value("enabled", TestValue::Bool(true))
}
