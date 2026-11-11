// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::thread;

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{WEAPON_COMPONENT, fire_actions};

const BASE: &str = "SWeaponActionParams";
const LEAF: &str = "SWeaponActionFireSingleParams";
const PARALLEL: &str = "SWeaponActionParallelParams";
const SEQUENCE: &str = "SWeaponActionSequenceParams";
const ENTRY: &str = "SWeaponSequenceEntryParams";
const NESTED_WRAPPERS: usize = 4096;
const TRAVERSAL_TEST_STACK_BYTES: usize = 512 * 1024;

#[test]
fn deeply_nested_legacy_weapon_actions_keep_authored_order_on_a_small_stack()
-> Result<(), Box<dyn Error>> {
    let bytes = fixture().build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("original weapon Record")?.tree()?;
    let actions = thread::scope(|scope| -> Result<_, Box<dyn Error>> {
        let worker = thread::Builder::new()
            .stack_size(TRAVERSAL_TEST_STACK_BYTES)
            .spawn_scoped(scope, || fire_actions(tree.root()))?;
        Ok(worker
            .join()
            .map_err(|_| "legacy action traversal thread")?)
    })?;
    assert_eq!(
        actions.iter().map(|action| action.name).collect::<Vec<_>>(),
        [Some("deep"), Some("sequenceSibling"), Some("rootSibling")],
        "depth-first flattening retains authored sequence and root order"
    );
    Ok(())
}

fn fixture() -> DataForgeBuilder {
    let mut builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(BASE))
        .structure(
            TestStruct::new(LEAF)
                .parent(BASE)
                .field("name", TestType::String),
        )
        .structure(
            TestStruct::new(PARALLEL)
                .parent(BASE)
                .array("weaponActions", pointer()),
        )
        .structure(TestStruct::new(ENTRY).field("weaponAction", pointer()))
        .structure(
            TestStruct::new(SEQUENCE)
                .parent(BASE)
                .array("sequenceEntries", TestType::Class(ENTRY.to_owned())),
        )
        .structure(TestStruct::new(WEAPON_COMPONENT).array("fireActions", pointer()));
    for name in ["deep", "sequenceSibling", "rootSibling"] {
        builder = builder.instance(
            name,
            TestInstance::new(LEAF).value("name", TestValue::text(name)),
        );
    }
    let mut next = "deep".to_owned();
    for index in (0..NESTED_WRAPPERS).rev() {
        let name = format!("parallel{index}");
        builder = builder.instance(
            &name,
            TestInstance::new(PARALLEL).value(
                "weaponActions",
                TestValue::Array(vec![TestValue::pointer(next)]),
            ),
        );
        next = name;
    }
    builder.record(
        TestRecord::new(
            1,
            WEAPON_COMPONENT,
            "Fixture.DeepWeapon",
            "fixture/deep-weapon.xml",
        )
        .value(
            "fireActions",
            TestValue::Array(vec![
                TestValue::Struct(TestInstance::new(SEQUENCE).value(
                    "sequenceEntries",
                    TestValue::Array(vec![entry(next), entry("sequenceSibling")]),
                )),
                TestValue::pointer("rootSibling"),
            ]),
        ),
    )
}

fn pointer() -> TestType {
    TestType::StrongPointer(BASE.to_owned())
}

fn entry(next: impl Into<String>) -> TestValue {
    TestValue::Struct(TestInstance::new(ENTRY).value("weaponAction", TestValue::pointer(next)))
}
