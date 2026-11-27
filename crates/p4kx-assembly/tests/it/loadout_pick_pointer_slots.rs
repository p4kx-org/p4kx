// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{LoadoutChanges, LoadoutError, LoadoutPicks, LoadoutResolver};
use p4kx_dataforge::{DataForge, Value};
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestRecord, TestValue, entity_schema,
};

use super::TestResult;
use crate::loadout_assortment::{ASSORTMENT, attached, generated, pair};

#[test]
fn default_and_named_picks_keep_physical_pointer_slots() -> TestResult {
    let bytes = data();
    let forge = DataForge::open(&bytes)?;
    check_pointer_slots(&forge)?;

    let root = forge.record(0).ok_or("synthetic root missing")?;
    let mut resolver = LoadoutResolver::new(&forge);
    for (pick, issue, body) in [
        (None, Some("the first alternative"), None),
        (Some(1), None, Some("Body_Red")),
        (Some(2), Some("the alternative 2"), None),
        (Some(3), None, Some("Body_Blue")),
    ] {
        let node = match pick {
            None => resolver.resolve(&root)?,
            Some(index) => {
                let changes =
                    LoadoutChanges::new().with_picks(LoadoutPicks::new().group("Body", index));
                resolver.resolve_with(&root, &changes)?
            }
        };
        if let Some(issue) = issue {
            assert!(
                matches!(node.issues(), [LoadoutError::Assortment { reason, .. }]
                    if reason.contains(issue)),
                "pick {pick:?} records its own group slot"
            );
        } else {
            assert!(node.issues().is_empty(), "pick {pick:?} remains in range");
        }
        let expected: Vec<_> = body
            .map(|class| pair("Body_ItemPort", class))
            .into_iter()
            .chain([pair("Egg_ItemPort", "Egg")])
            .collect();
        assert_eq!(attached(&node), expected, "physical pick {pick:?}");
    }
    Ok(())
}

fn check_pointer_slots(forge: &DataForge<'_>) -> TestResult {
    let record = forge
        .records()
        .find(|record| record.type_name().ok() == Some("SLoadoutAssortment"))
        .ok_or("synthetic assortment missing")?;
    let tree = record.tree()?;
    let Some(Value::Array(groups)) = tree.field("GroupedLoadouts") else {
        return Err("expected authored groups".into());
    };
    let Some(Value::Struct(body)) = groups.first() else {
        return Err("expected authored Body group".into());
    };
    let Some(Value::Array(loadouts)) = body.field("Loadouts") else {
        return Err("expected authored alternative pointers".into());
    };
    assert!(
        matches!(
            loadouts.as_ref(),
            [
                Value::Null,
                Value::Struct(_),
                Value::Error(_),
                Value::Struct(_)
            ]
        ),
        "the decoder keeps both failed pointer positions"
    );
    Ok(())
}

fn data() -> Vec<u8> {
    let mut broken_pointer = (u32::MAX - 1).to_le_bytes().to_vec();
    broken_pointer.extend_from_slice(&0_u32.to_le_bytes());
    let body = group(
        "Body",
        vec![
            TestValue::Null,
            manual("Body_ItemPort", "Body_Red"),
            TestValue::Raw(broken_pointer),
            manual("Body_ItemPort", "Body_Blue"),
        ],
    );
    let egg = group("Egg", vec![manual("Egg_ItemPort", "Egg")]);
    let assortment = TestRecord::new(
        ASSORTMENT,
        "SLoadoutAssortment",
        "SLoadoutAssortment.PointerSlots",
        "synthetic/pointer_slots.xml",
    )
    .value("GroupedLoadouts", TestValue::Array(vec![body, egg]));

    entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "PointerSlots", "synthetic/root.xml")
                .loadout(generated(&[ASSORTMENT]))
                .record(),
        )
        .record(TestEntity::new(2, "Body_Red", "synthetic/red.xml").record())
        .record(TestEntity::new(3, "Body_Blue", "synthetic/blue.xml").record())
        .record(TestEntity::new(4, "Egg", "synthetic/egg.xml").record())
        .record(assortment)
        .build()
}

fn group(name: &str, alternatives: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SGroupedLoadouts")
            .value("Group", TestValue::text(name))
            .value("Loadouts", TestValue::Array(alternatives)),
    )
}

fn manual(port: &str, class: &str) -> TestValue {
    let attachment = TestInstance::new("SItemPortLoadoutEntryParams")
        .value("itemPortName", TestValue::text(port))
        .value("entityClassName", TestValue::text(class));
    TestValue::Struct(TestInstance::new("SItemPortLoadoutManualParams").value(
        "entries",
        TestValue::Array(vec![TestValue::Struct(attachment)]),
    ))
}
