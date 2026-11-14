// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestLoadout, TestLoadoutEntry, TestPort, entity_schema,
};

const GUID: u128 = 0x6b00_0000_0000_4000_8000_0000_0000_0001;

pub(crate) fn database() -> Vec<u8> {
    let mut builder = entity_schema(DataForgeBuilder::new());
    for (name, size) in [("First", 1), ("Second", 2)] {
        builder = builder.record(
            TestEntity::new(GUID, name, format!("synthetic/{name}.xml"))
                .attach_type("Char_Armor_Torso")
                .size(size)
                .geometry(format!("{name}.cgf"), None)
                .geometry_variant("Male", format!("{name}-male.cgf"), None)
                .record(),
        );
    }
    builder
        .record(
            TestEntity::new(GUID + 1, "Holder", "synthetic/holder.xml")
                .port(TestPort::new("slot").flags("Male"))
                .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
                    "slot", "Second",
                )]))
                .record(),
        )
        .build()
}
