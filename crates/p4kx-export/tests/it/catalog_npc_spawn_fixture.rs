// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const ARCHETYPE: u128 = 0x1084_0001;
pub(super) const POOL: u128 = 0x1084_0002;
const EMPTY_POOL: u128 = 0x1084_0003;
pub(super) const MISSING: u128 = 0x1084_00ff;

pub(super) fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(TestStruct::new("FactionIdentity"))
        .structure(
            TestStruct::new("NPCClassPoolEntry")
                .field("weight", TestType::Single)
                .field("npcClass", reference("EntityClassDefinition")),
        )
        .structure(
            TestStruct::new("NPCClassPool")
                .array("npcClasses", TestType::Class("NPCClassPoolEntry".into())),
        )
        .structure(
            TestStruct::new("NPCIdentityToArchetype")
                .field("identity", reference("FactionIdentity"))
                .array("npcPools", reference("NPCClassPool")),
        )
        .structure(
            TestStruct::new("NPCArchetype")
                .array("identity", TestType::Class("NPCIdentityToArchetype".into())),
        )
        .record(
            record(EMPTY_POOL, "NPCClassPool", "pools/empty")
                .value("npcClasses", TestValue::Array(Vec::new())),
        )
        .record(record(POOL, "NPCClassPool", "pools/shared").value(
            "npcClasses",
            TestValue::Array(vec![
                pool_entry(-0.0, TestValue::Null),
                pool_entry(1.25, TestValue::Reference(MISSING)),
            ]),
        ))
        .record(
            record(ARCHETYPE, "NPCArchetype", "archetypes/primary").value(
                "identity",
                TestValue::Array(vec![
                    identity_branch(TestValue::Null, vec![TestValue::Reference(POOL); 2]),
                    identity_branch(
                        TestValue::Reference(MISSING),
                        vec![TestValue::Reference(POOL)],
                    ),
                ]),
            ),
        )
}

pub(super) fn record(guid: u128, source_type: &str, suffix: &str) -> TestRecord {
    TestRecord::new(
        guid,
        source_type,
        format!("{source_type}.Fixture"),
        format!("libs/foundry/records/npc_fixture/{suffix}.xml"),
    )
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

fn pool_entry(weight: f32, npc_class: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new("NPCClassPoolEntry")
            .value("weight", TestValue::Single(weight))
            .value("npcClass", npc_class),
    )
}

fn identity_branch(identity: TestValue, pools: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("NPCIdentityToArchetype")
            .value("identity", identity)
            .value("npcPools", TestValue::Array(pools)),
    )
}
