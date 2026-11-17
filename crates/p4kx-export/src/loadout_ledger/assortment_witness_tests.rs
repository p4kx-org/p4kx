// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestEntity, TestLoadout, assortment_record, entity_schema};

use super::{DataForge, Kind, LoadoutResolver, Result, run};

const WRONG_TYPE: u128 = 0x31;
const NO_LOADOUT: u128 = 0x32;
const SELF_CYCLE: u128 = 0x33;

#[test]
fn catalog_root_policy_and_default_picks_distinguish_assortment_failures() -> Result {
    let bytes = witness_data();
    let forge = DataForge::open(&bytes)?;
    let census = run(
        &forge,
        LoadoutResolver::new(&forge).stopping_at_docked_vehicles(),
    );
    assert_eq!(
        census.roots, 4,
        "admit ships and items with ports or a default Loadout"
    );
    assert_eq!(census.resolution_failures, 0);
    assert_eq!(census.matches.len(), 3, "only three assortment failures");
    for (match_position, kind, physical_target, reason) in [
        (0, Kind::Ships, 7, "not an assortment"),
        (1, Kind::Items, 8, "the first alternative"),
        (2, Kind::Items, 9, "names itself"),
    ] {
        let finding = &census.matches[match_position];
        assert_eq!(finding.kind, kind);
        assert_eq!(finding.root_index, u32::try_from(match_position)?);
        assert_eq!(finding.target_index, Some(physical_target));
        assert!(
            finding.reason.contains(reason),
            "cause: {:?}",
            finding.reason
        );
    }
    Ok(())
}

fn witness_data() -> Vec<u8> {
    let wrong = "libs/foundry/records/entities/spaceships/synthetic_ship.xml";
    let items = "libs/foundry/records/entities/scitem";
    let generated = |target| TestLoadout::RandomGenerated(vec![target]);
    entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "SyntheticShip", wrong)
                .loadout(generated(WRONG_TYPE))
                .record(),
        )
        .record(default_item(2, items, "SyntheticEmpty", NO_LOADOUT).record())
        .record(default_item(3, items, "SyntheticCycle", SELF_CYCLE).record())
        .record(default_item(4, items, "SyntheticDangling", 0x40).record())
        .record(
            default_item(
                5,
                "libs/foundry/records/entities/npcs",
                "SyntheticNpc",
                WRONG_TYPE,
            )
            .record(),
        )
        .record(TestEntity::new(6, "NoPorts", format!("{items}/no_ports.xml")).record())
        .record(TestEntity::new(7, "Other", "entities/other.xml").record())
        .record(TestEntity::new(WRONG_TYPE, "WrongType", "entities/wrong_type.xml").record())
        .record(assortment_record(
            NO_LOADOUT,
            "bad_alternative",
            &[(
                "Body",
                vec![
                    TestLoadout::Xml(String::new()),
                    TestLoadout::Manual(Vec::new()),
                ],
            )],
        ))
        .record(assortment_record(
            SELF_CYCLE,
            "self_cycle",
            &[("Body", vec![generated(SELF_CYCLE)])],
        ))
        .record(default_item(8, items, "test_unused", WRONG_TYPE).record())
        .build()
}

fn default_item(id: u128, folder: &str, class: &str, target: u128) -> TestEntity {
    TestEntity::new(id, class, format!("{folder}/{class}.xml"))
        .loadout(TestLoadout::RandomGenerated(vec![target]))
}
