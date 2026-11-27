// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(crate) const POOL: u128 = 1;
pub(crate) const OWNER: u128 = 3;
pub(crate) const ALIAS: u32 = 1;
pub(crate) const SOURCE: u32 = 3;
pub(crate) const SOURCE_ALIAS: u32 = 4;
pub(crate) const WIDE_CHANCE: u64 = 9_007_199_254_740_993;

pub(crate) fn database(broken: bool) -> DataForgeBuilder {
    let builder = schema()
        .record(pool(POOL, "Primary", "primary"))
        .record(pool(POOL, "Alias", "alias"))
        .record(pool(2, "Unrelated", "unrelated"))
        .record(
            source("Source.Primary", "outside/first.xml")
                .value("direct", TestValue::Reference(POOL))
                .value("rootWeak", TestValue::pointer("BlueprintPoolRecord.Alias"))
                .value("other", TestValue::Reference(9))
                .value("raw", TestValue::Array(vec![TestValue::Reference(POOL)]))
                .value("nested", TestValue::Array(vec![reward(), reward()])),
        )
        .record(
            source("Source.Alias", "outside/second.xml")
                .value("direct", TestValue::Reference(POOL)),
        )
        .record(
            TestRecord::new(POOL, "Source", "Source.Self", "outside/self.xml")
                .value("direct", TestValue::Reference(POOL)),
        );
    if broken {
        builder
            .record(TestRecord::new(
                8,
                "Broken",
                "Broken.Source",
                "outside/broken.xml",
            ))
            .record(TestRecord::new(
                9,
                "Broken",
                "Broken.Foreign",
                "outside/foreign.xml",
            ))
    } else {
        builder
            .record(TestRecord::new(
                8,
                "Empty",
                "Empty.Source",
                "outside/empty.xml",
            ))
            .record(TestRecord::new(
                9,
                "Empty",
                "Empty.Foreign",
                "outside/foreign.xml",
            ))
    }
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("BlueprintPoolRecord"))
        .structure(TestStruct::new("Empty"))
        .structure(
            TestStruct::new("Broken")
                .field("bad", TestType::UInt64)
                .declared_size(1),
        )
        .structure(
            TestStruct::new("Reward")
                .field("pool", TestType::Reference)
                .field("chance", TestType::Single),
        )
        .structure(
            TestStruct::new("Source")
                .field("id", TestType::Guid)
                .field("debugName", TestType::String)
                .field("direct", TestType::Reference)
                .field(
                    "rootWeak",
                    TestType::WeakPointer("BlueprintPoolRecord".to_owned()),
                )
                .field("other", TestType::Reference)
                .array("raw", TestType::Reference)
                .array("nested", TestType::Class("Reward".to_owned()))
                .field("chance", TestType::UInt64),
        )
}

fn pool(guid: u128, name: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "BlueprintPoolRecord",
        format!("BlueprintPoolRecord.{name}"),
        format!("libs/foundry/records/crafting/blueprintrewards/{path}.xml"),
    )
}

fn source(name: &str, path: &str) -> TestRecord {
    TestRecord::new(OWNER, "Source", name, path)
        .value("id", TestValue::Guid(31))
        .value("debugName", TestValue::text("Authored source identity"))
        .value("chance", TestValue::UInt64(WIDE_CHANCE))
}

fn reward() -> TestValue {
    TestValue::Struct(
        TestInstance::new("Reward")
            .value("pool", TestValue::Reference(POOL))
            .value("chance", TestValue::Single(0.25)),
    )
}
