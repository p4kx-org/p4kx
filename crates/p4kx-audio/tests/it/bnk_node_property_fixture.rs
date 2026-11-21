// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_audio::{HircKind, HircLayout, Soundbank};
use p4kx_testkit::{TestBank, TestHircObject};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(super) const OBJECT_ID: u32 = 0xB380_1471;
pub(super) const PARENT_ID: u32 = 0x5632_90FA;
pub(super) const OVERRIDE_BUS_ID: u32 = 0x7890_1234;
pub(super) const PROPERTY_IDS: [u8; 3] = [0xF1, 0x03, 0xF1];
pub(super) const PROPERTY_BITS: [u32; 3] = [0x7FC1_2345, 0, 0x8000_0000];
pub(super) const RANGED_IDS: [u8; 2] = [0x03, 0xFA];
pub(super) const RANGED_BITS: [u32; 4] = [0, 0xFF80_0000, 0x0100_0001, 0x7FC0_0019];

#[derive(Clone, Copy)]
pub(super) enum Owner {
    Sound,
    ActorMixer,
}

impl Owner {
    pub(super) const fn kind(self) -> HircKind {
        match self {
            Self::Sound => HircKind::Sound,
            Self::ActorMixer => HircKind::ActorMixer,
        }
    }

    const fn type_code(self) -> u8 {
        match self {
            Self::Sound => 2,
            Self::ActorMixer => 7,
        }
    }

    pub(super) const fn source_header_len(self) -> usize {
        match self {
            Self::Sound => 18,
            Self::ActorMixer => 0,
        }
    }
}

pub(super) fn complete(owner: Owner, version: u32, override_bus: u32) -> Vec<u8> {
    let mut lists = bundle(&PROPERTY_IDS, &PROPERTY_BITS);
    lists.extend(bundle(&RANGED_IDS, &RANGED_BITS));
    lists.extend([0, 0]);
    lists.extend(0_u32.to_le_bytes());
    lists.extend([0, 1, 0, 0, 3, 0x0C]);
    lists.extend([0, 0]);
    lists.extend(0_u16.to_le_bytes());
    if matches!(owner, Owner::ActorMixer) {
        lists.extend(0_u32.to_le_bytes());
    }
    bank(owner, version, override_bus, &lists)
}

pub(super) fn bundle(ids: &[u8], words: &[u32]) -> Vec<u8> {
    let mut bytes = vec![u8::try_from(ids.len()).unwrap_or(u8::MAX)];
    bytes.extend(ids);
    bytes.extend(words.iter().copied().flat_map(u32::to_le_bytes));
    bytes
}

pub(super) fn bank(owner: Owner, version: u32, override_bus: u32, lists: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    if matches!(owner, Owner::Sound) {
        body.extend(0x0004_0001_u32.to_le_bytes());
        body.push(2);
        body.extend([1_u32, 2, 0].into_iter().flat_map(u32::to_le_bytes));
        body.push(0);
    }
    body.extend([0, 0, 0, 0]);
    body.extend(override_bus.to_le_bytes());
    body.extend(PARENT_ID.to_le_bytes());
    body.push(0);
    body.extend(lists);
    TestBank::new(4)
        .version(version)
        .object(TestHircObject::raw(owner.type_code(), OBJECT_ID, body))
        .object(TestHircObject::event(1, &[]))
        .bytes()
}

pub(super) fn assert_layout(bytes: &[u8], owner: Owner, expected: HircLayout) -> TestResult {
    let bank = Soundbank::parse(bytes)?;
    let object = bank.objects().first().ok_or("original HIRC node")?;
    assert_eq!(object.id, OBJECT_ID, "the original node occurrence");
    assert_eq!(object.kind, owner.kind(), "the original node kind");
    assert_eq!(object.layout(), expected, "the original node layout");
    Ok(())
}
