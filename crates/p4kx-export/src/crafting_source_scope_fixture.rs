// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::fixture::{self, A_GUID, B_GUID, LEAF, POOL, ROOT_GUID};

pub(super) const BASE: &str = "ScopeBase";
pub(super) const NAME: &str = "NameScope";
pub(super) const ID: &str = "IdScope";
pub(super) const ROOT: &str = "ScopeRoot";
const FIRST_ID: u128 = 0x5555_5555_5555_5555_5555_5555_5555_5555;
const NESTED_ID: u128 = 0x6666_6666_6666_6666_6666_6666_6666_6666;

fn node(kind: &str, name: &str, children: &[&str], reward: Option<&str>) -> TestInstance {
    TestInstance::new(kind)
        .value("debugName", TestValue::text(name))
        .value(
            "children",
            TestValue::Array(
                children
                    .iter()
                    .map(|name| TestValue::pointer(*name))
                    .collect(),
            ),
        )
        .value("reward", reward.map_or(TestValue::Null, TestValue::pointer))
}

pub(super) fn database() -> Vec<u8> {
    fixture::targets(rewards(nodes()).record(owner())).build()
}

fn nodes() -> DataForgeBuilder {
    schema()
        .instance("loose", node(NAME, "nearest", &[], Some("loose-reward")))
        .instance(
            "identity",
            node(ID, "bound", &["named", "nameless"], Some("identity-reward"))
                .value("id", TestValue::Guid(FIRST_ID)),
        )
        .instance("named", node(NAME, "misleading", &[], Some("named-reward")))
        .instance(
            "nameless",
            node(ID, "", &["nested"], None).value("id", TestValue::Guid(NESTED_ID)),
        )
        .instance("nested", node(NAME, "ignored", &[], Some("nested-reward")))
        .instance("later", node(NAME, "", &[], Some("later-reward")))
}

fn rewards(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .instance("loose-reward", fixture::reward(A_GUID, 0.5))
        .instance("named-reward", fixture::reward(B_GUID, 0.0))
        .instance("nested-reward", fixture::reward(A_GUID, 1.5))
        .instance("identity-reward", fixture::reward(B_GUID, 0.25))
        .instance("later-reward", fixture::reward(A_GUID, 0.75))
}

fn owner() -> TestRecord {
    TestRecord::new(ROOT_GUID, ROOT, "ScopeRoot.Source", "rewards/scopes.xml")
        .value("debugName", TestValue::text("root-fallback"))
        .value(
            "children",
            TestValue::Array(vec![
                TestValue::pointer("loose"),
                TestValue::Null,
                TestValue::pointer("identity"),
                TestValue::pointer("later"),
            ]),
        )
        .value(
            "arrayOnly",
            TestValue::Array(vec![
                TestValue::Reference(A_GUID),
                TestValue::Reference(B_GUID),
            ]),
        )
}

fn scoped(kind: &str) -> TestStruct {
    TestStruct::new(kind)
        .parent(BASE)
        .field("debugName", TestType::String)
        .array("children", TestType::StrongPointer(BASE.to_owned()))
        .field("reward", TestType::StrongPointer(LEAF.to_owned()))
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(BASE))
        .structure(scoped(NAME))
        .structure(scoped(ID).field("id", TestType::Guid))
        .structure(fixture::leaf())
        .structure(TestStruct::new(POOL))
        .structure(
            TestStruct::new(ROOT)
                .field("debugName", TestType::String)
                .array("children", TestType::StrongPointer(BASE.to_owned()))
                .array("arrayOnly", TestType::ReferenceTo(POOL.to_owned())),
        )
}
