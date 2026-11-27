// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestEntity, TestLoadout, TestLoadoutEntry, entity_schema};

use super::{ROOT, UNUSED};

pub(super) fn database() -> Vec<u8> {
    fn entity(id: u128, class: &str) -> TestEntity {
        TestEntity::new(id, class, format!("synthetic/{class}.xml"))
    }

    fn root(id: u128, class: &str, active: &str) -> TestEntity {
        entity(id, class).loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("branch", "Branch").loadout(TestLoadout::Manual(vec![
                TestLoadoutEntry::class("stale", "Inert"),
                TestLoadoutEntry::class("active", active),
            ])),
        ]))
    }

    entity_schema(DataForgeBuilder::new().version(8))
        .record(root(1, "RootA", "ActiveA").record())
        .record(root(2, "RootB", "ActiveB").record())
        .record(
            entity(3, "Branch")
                .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
                    "stale",
                    "Prefetched",
                )]))
                .record(),
        )
        .record(
            entity(4, "ActiveA")
                .loadout(TestLoadout::Xml(ROOT.to_owned()))
                .record(),
        )
        .record(
            entity(5, "ActiveB")
                .loadout(TestLoadout::Xml(ROOT.to_owned()))
                .record(),
        )
        .record(
            entity(6, "Prefetched")
                .loadout(TestLoadout::Xml(UNUSED.to_owned()))
                .record(),
        )
        .record(entity(7, "Cargo").attach_type("CargoGrid").record())
        .record(entity(8, "Inert").record())
        .build()
}
