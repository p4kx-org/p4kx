// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_testkit::{TestBank, TestHircObject};

pub(super) const SOUND_ID: u32 = 0x33D0_0042;
pub(super) const PARENT_ID: u32 = 0x1122_3344;
pub(super) const BINDING_ID: u32 = 258;
pub(super) const GROUP_ID: u32 = 0xE175_C001;
pub(super) const STATE_ID: u32 = 0x1234_5678;
pub(super) const PROPERTY_IDS: [u16; 2] = [0xF302, 7];
pub(super) const VALUE_BITS: [u32; 2] = [0x7FC1_2345, 0x8000_0000];

pub(super) fn state_bytes() -> Vec<u8> {
    let mut bytes = vec![2, 0x82, 0x02, 0xF0, 0x81, 0x82, 0x02, 0, 1, 2];
    bytes.extend(GROUP_ID.to_le_bytes());
    bytes.extend([0xE1, 2]);
    bytes.extend(STATE_ID.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(PROPERTY_IDS.into_iter().flat_map(u16::to_le_bytes));
    bytes.extend(VALUE_BITS.into_iter().flat_map(u32::to_le_bytes));
    bytes.extend(STATE_ID.to_le_bytes());
    bytes.extend(0_u16.to_le_bytes());
    bytes.extend(GROUP_ID.to_le_bytes());
    bytes.extend([0, 0]);
    bytes
}

pub(super) struct Fixture {
    pub(super) bytes: Vec<u8>,
    pub(super) state_in_body: Range<usize>,
}

pub(super) fn sound_bank(version: u32, states: &[u8], rtpc: &[u8]) -> Fixture {
    let mut body = source_body();
    body.extend([0, 0, 0, 0]);
    body.extend(7_u32.to_le_bytes());
    body.extend(PARENT_ID.to_le_bytes());
    body.extend([0, 0, 0, 0, 0]);
    body.extend(0_u32.to_le_bytes());
    body.extend([0, 1, 0, 0, 3, 0x0C]);
    let start = size_of::<u32>() + body.len();
    body.extend(states);
    body.extend(rtpc);
    Fixture {
        bytes: TestBank::new(77)
            .version(version)
            .object(TestHircObject::raw(2, SOUND_ID, body))
            .object(TestHircObject::event(8, &[9]))
            .bytes(),
        state_in_body: start..start + states.len(),
    }
}

pub(super) fn rtpc_bytes() -> Vec<u8> {
    let mut rtpc = 1_u16.to_le_bytes().to_vec();
    rtpc.extend(0xFA01_8888_u32.to_le_bytes());
    rtpc.extend([1, 2, 0x81, 0x07]);
    rtpc.extend(0x33EA_88FF_u32.to_le_bytes());
    rtpc.push(4);
    rtpc.extend(1_u16.to_le_bytes());
    rtpc.extend([0; 12]);
    rtpc
}

pub(super) fn source_body() -> Vec<u8> {
    let mut body = 0x0004_0001_u32.to_le_bytes().to_vec();
    body.push(2);
    body.extend(1_u32.to_le_bytes());
    body.extend(2_u32.to_le_bytes());
    body.extend(0_u32.to_le_bytes());
    body.push(0);
    body
}
