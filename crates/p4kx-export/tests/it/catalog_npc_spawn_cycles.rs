// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

const FIRST: u128 = 0x1084_0201;
const SECOND: u128 = 0x1084_0202;

#[test]
fn npc_definition_source_arena_keeps_reference_cycles_without_expansion() -> TestResult {
    let source_type = "NPCClassPool";
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(source_type).field(
            "fixtureLinkedPool",
            TestType::ReferenceTo(source_type.to_owned()),
        ))
        .record(pool(FIRST, SECOND, "first"))
        .record(pool(SECOND, FIRST, "second"));
    let (_directory, archive) = common::open(&database, DcbOrigin::Plain)?;
    let options = common::options();
    let first = build_catalog(&archive, &options)?;
    let repeated = build_catalog(&archive, &options)?;
    assert_eq!(
        first.json(),
        repeated.json(),
        "source links are deterministic"
    );
    let catalog: Value = serde_json::from_slice(first.json())?;
    for (index, target) in [(0, SECOND), (1, FIRST)] {
        let row = at(
            &catalog,
            &format!("/definitions/npcClassPools/rows/{index}"),
        )?;
        let fields = at(row, "/authoredFields")?;
        let nodes = at(fields, "/values")?
            .as_array()
            .ok_or("physical source arena")?;
        assert_eq!(
            nodes.len(),
            1,
            "one source occurrence, independent of target depth"
        );
        assert_eq!(
            at(row, "/npcClasses/state")?,
            "absent",
            "no invented class array"
        );
        assert_eq!(
            at(fields, "/values/0/value/value/value/guid/value")?,
            &json!(guid(target)),
            "cyclic references retain their physical targets"
        );
    }
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

fn pool(id: u128, target: u128, label: &str) -> TestRecord {
    TestRecord::new(
        id,
        "NPCClassPool",
        format!("NPCClassPool.Fixture{label}"),
        format!("libs/foundry/records/npc_fixture/cycles/{label}.xml"),
    )
    .value("fixtureLinkedPool", TestValue::Reference(target))
}
