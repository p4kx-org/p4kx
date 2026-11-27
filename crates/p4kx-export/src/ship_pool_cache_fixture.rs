// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry, TestLoadout};

pub(super) const PATH: &str = "Data/synthetic/shared.xml";
pub(super) const SELECTOR: &str = "synthetic/shared.xml";

#[derive(Debug, Clone, Copy)]
pub(super) enum Case {
    Valid,
    Malformed,
    Unreadable,
    Missing,
}

impl Case {
    pub(super) const fn read_ok(self) -> bool {
        matches!(self, Self::Valid | Self::Malformed)
    }

    pub(super) const fn projected(self) -> Option<bool> {
        match self {
            Self::Valid => Some(true),
            Self::Malformed => Some(false),
            Self::Unreadable | Self::Missing => None,
        }
    }

    pub(super) fn entries(self) -> Vec<TestEntry> {
        match self {
            Self::Valid => vec![TestEntry::stored(PATH, b"<Loadout/>".as_slice())],
            Self::Malformed => vec![TestEntry::stored(PATH, b"<broken>".as_slice())],
            Self::Unreadable => {
                vec![TestEntry::zstd(PATH, b"<Loadout/>".as_slice()).with_payload(vec![0, 1, 2])]
            }
            Self::Missing => Vec::new(),
        }
    }
}

pub(super) fn database() -> Vec<u8> {
    let mut database = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8));
    for (index, guid) in [(0, 0x5731), (1, 0x5732)] {
        database = database.record(
            TestEntity::new(
                guid,
                format!("CacheShip{index}"),
                format!("libs/foundry/records/entities/spaceships/cache{index}.xml"),
            )
            .attach_type("NOITEM_Vehicle")
            .loadout(TestLoadout::Xml(SELECTOR.to_owned()))
            .record(),
        );
    }
    database.build()
}
