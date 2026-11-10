// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestHircObject};

pub(crate) const TRIGGER: &str = "Complete_Stingers";
pub(crate) const EVENT: &str = "Wwise_Complete_Stingers";
pub(crate) const LATE: u32 = 0x77;
pub(crate) const COMMON: u32 = 0x88;
pub(crate) const HOLDER: u32 = 20;
pub(crate) const TARGET: u32 = 21;
pub(crate) const TRACK: u32 = 31;
pub(crate) const WEM: u32 = 7000;
pub(crate) const ACTION: u32 = 10;
pub(crate) const FIRST_PREFIX: u32 = 0x1000_0000;

#[derive(Clone, Copy)]
pub(crate) enum Case {
    SingleBoundary,
    GlobalBoundary,
    Duplicates,
    Missing,
    Small,
}

pub(crate) struct BankSource {
    pub(crate) path: &'static str,
    pub(crate) id: u32,
    pub(crate) prefix: u32,
    pub(crate) tails: u32,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Clone, Copy)]
enum Role {
    Prefix,
    Playable,
}

pub(crate) fn banks(case: Case) -> Vec<BankSource> {
    match case {
        Case::SingleBoundary => vec![bank(Role::Playable, 262_144, 1)],
        Case::GlobalBoundary => vec![
            bank(Role::Prefix, 131_072, 0),
            bank(Role::Playable, 131_072, 1),
        ],
        Case::Duplicates => vec![bank(Role::Prefix, 0, 2), bank(Role::Playable, 0, 2)],
        Case::Missing => vec![bank(Role::Playable, 2, 0)],
        Case::Small => vec![bank(Role::Playable, 2, 1)],
    }
}

fn bank(role: Role, prefix: u32, tails: u32) -> BankSource {
    let mut stingers: Vec<_> = (0..prefix)
        .map(|index| (COMMON, FIRST_PREFIX + index))
        .collect();
    stingers.extend((0..tails).map(|_| (LATE, TARGET)));
    let (path, id) = match role {
        Role::Prefix => ("Data\\Sounds\\wwise\\A-prefix.bnk", 0xB0),
        Role::Playable => ("Data\\Sounds\\wwise\\Z-tail.bnk", 0xB1),
    };
    let mut bank = TestBank::new(id).object(TestHircObject::music_segment_with_stingers(
        HOLDER,
        &[],
        &stingers,
    ));
    if matches!(role, Role::Playable) {
        bank = bank
            .object(TestHircObject::event(
                p4kx_testkit::wwise_short_id(EVENT),
                &[ACTION],
            ))
            .object(TestHircObject::action(ACTION, 0x1B03, LATE));
    }
    if tails > 0 {
        bank = bank
            .object(TestHircObject::music_segment_with_stingers(
                TARGET,
                &[TRACK],
                &[],
            ))
            .object(TestHircObject::music_track(TRACK, &[WEM]));
    }
    BankSource {
        path,
        id,
        prefix,
        tails,
        bytes: bank.bytes(),
    }
}
