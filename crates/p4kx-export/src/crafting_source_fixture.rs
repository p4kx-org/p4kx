// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::Guid;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const ROOT_GUID: u128 = 0x1111_1111_1111_1111_1111_1111_1111_1111;
pub(super) const A_GUID: u128 = 0x2222_2222_2222_2222_2222_2222_2222_2222;
pub(super) const B_GUID: u128 = 0x3333_3333_3333_3333_3333_3333_3333_3333;
const OTHER_GUID: u128 = 0x4444_4444_4444_4444_4444_4444_4444_4444;
pub(super) const NODE: &str = "RewardNode";
pub(super) const LEAF: &str = "RewardLeaf";
pub(super) const ROOT: &str = "RewardRoot";
pub(super) const POOL: &str = "RewardPool";

pub(super) const fn pool_a() -> Guid {
    Guid::from_stored([0x22; 16])
}
pub(super) const fn pool_b() -> Guid {
    Guid::from_stored([0x33; 16])
}

pub(super) fn pools() -> HashSet<Guid> {
    HashSet::from([pool_a(), pool_b(), Guid::from_stored([0x11; 16])])
}

fn name(index: u32) -> String {
    format!("reward-node-{index}")
}

pub(super) fn leaf() -> TestStruct {
    TestStruct::new(LEAF)
        .field("first", TestType::ReferenceTo(POOL.to_owned()))
        .field("duplicate", TestType::ReferenceTo(POOL.to_owned()))
        .field("second", TestType::ReferenceTo(POOL.to_owned()))
        .field("chance", TestType::Single)
}

pub(super) fn reward(pool: u128, chance: f32) -> TestInstance {
    TestInstance::new(LEAF)
        .value("first", TestValue::Reference(pool))
        .value("duplicate", TestValue::Null)
        .value("second", TestValue::Null)
        .value("chance", TestValue::Single(chance))
}

pub(super) fn targets(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .record(TestRecord::new(
            A_GUID,
            POOL,
            "RewardPool.A",
            "rewards/pool-a.xml",
        ))
        .record(TestRecord::new(
            B_GUID,
            POOL,
            "RewardPool.B",
            "rewards/pool-b.xml",
        ))
        .record(TestRecord::new(
            OTHER_GUID,
            POOL,
            "RewardPool.Other",
            "rewards/other.xml",
        ))
}

pub(super) fn database(depth: u32) -> Vec<u8> {
    let mut builder = schema();
    for index in 0..depth {
        let terminal = index + 1 == depth;
        let next = if terminal {
            TestValue::Null
        } else {
            TestValue::pointer(name(index + 1))
        };
        let reward = if terminal {
            TestValue::pointer("tail")
        } else {
            TestValue::Null
        };
        builder = builder.instance(
            name(index),
            TestInstance::new(NODE)
                .value("mark", TestValue::UInt32(index + 1))
                .value("next", next)
                .value("reward", reward),
        );
    }
    builder = builder
        .instance(
            "tail",
            reward(A_GUID, 0.25)
                .value("duplicate", TestValue::Reference(A_GUID))
                .value("second", TestValue::Reference(B_GUID)),
        )
        .instance("after", reward(B_GUID, 0.75));
    targets(builder.record(owner(depth))).build()
}

fn owner(depth: u32) -> TestRecord {
    TestRecord::new(ROOT_GUID, ROOT, "RewardRoot.Source", "rewards/deep.xml")
        .value("debugName", TestValue::text("root"))
        .value("chain", TestValue::pointer(name(0)))
        .value("after", TestValue::pointer("after"))
        .value("self", TestValue::Reference(ROOT_GUID))
        .value("rootWeak", TestValue::pointer("RewardRoot.Source"))
        .value("local", TestValue::pointer(name(depth - 1)))
        .value("ignore", TestValue::Reference(OTHER_GUID))
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(NODE)
                .field("mark", TestType::UInt32)
                .field("next", TestType::StrongPointer(NODE.to_owned()))
                .field("reward", TestType::StrongPointer(LEAF.to_owned())),
        )
        .structure(leaf())
        .structure(
            TestStruct::new(ROOT)
                .field("debugName", TestType::String)
                .field("chain", TestType::StrongPointer(NODE.to_owned()))
                .field("after", TestType::StrongPointer(LEAF.to_owned()))
                .field("self", TestType::ReferenceTo(ROOT.to_owned()))
                .field("rootWeak", TestType::WeakPointer(ROOT.to_owned()))
                .field("local", TestType::WeakPointer(NODE.to_owned()))
                .field("ignore", TestType::ReferenceTo(POOL.to_owned())),
        )
        .structure(TestStruct::new(POOL))
}
