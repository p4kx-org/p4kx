// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::TestHircObject;
use crate::bnk::variable_length;
use crate::bytes::{put_u16, put_u32};

const PLUGIN_TYPE_MASK: u32 = 0x0F;
const SOURCE_PLUGIN_TYPE: u32 = 2;
const PARENT_ID: u32 = 0x2C21_1B2E;
const PLAYLIST_WEIGHT: u32 = 50_000;

/// The kind of a synthetic container object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestContainerKind {
    /// A random container, type 5 in random mode.
    Random,
    /// A sequence container, type 5 in sequence mode.
    Sequence,
    /// A switch container, type 6.
    Switch,
    /// A blend container, type 9.
    Blend,
}

/// How much of the node base parameters a synthetic container writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestNodeBase {
    /// Every list empty and every flag clear, as most live containers are.
    Plain,
    /// One entry in every list and every optional block present: effects, metadata, properties,
    /// 3D automation, auxiliary sends, state properties and groups, and an RTPC curve.
    Full,
}

impl TestHircObject {
    /// A v154 `ActorMixer` with ordered physical child IDs.
    #[must_use]
    pub fn actor_mixer(id: u32, children: &[u32], base: TestNodeBase) -> Self {
        let mut body = node_base(base);
        put_children(&mut body, children);
        Self::raw(7, id, body)
    }

    /// A container object of `kind` with `children`, laid out as bank version 154 writes it.
    #[must_use]
    pub fn container(
        kind: TestContainerKind,
        id: u32,
        children: &[u32],
        base: TestNodeBase,
    ) -> Self {
        let mut body = node_base(base);
        let type_byte = match kind {
            TestContainerKind::Random | TestContainerKind::Sequence => {
                random_sequence(&mut body, kind == TestContainerKind::Sequence, children);
                5
            }
            TestContainerKind::Switch => {
                switch(&mut body, children);
                6
            }
            TestContainerKind::Blend => {
                blend(&mut body, children);
                9
            }
        };
        Self::raw(type_byte, id, body)
    }

    /// A Sound object whose source is the plug-in `0x0065_0002`, which generates its sound and
    /// has no WEM.
    #[must_use]
    pub fn plugin_sound(id: u32, source_id: u32) -> Self {
        Self::sound_of_plugin(id, source_id, 0x0065_0002)
    }

    /// A Sound object whose source is the External Source plug-in `0x0008_0001`, which the game
    /// resolves at runtime and no bank holds.
    #[must_use]
    pub fn external_sound(id: u32, source_id: u32) -> Self {
        Self::sound_of_plugin(id, source_id, 0x0008_0001)
    }

    /// A Sound object whose source is the MIDI plug-in `0x0010_0001`, stored in a bank.
    #[must_use]
    pub fn midi_sound(id: u32, source_id: u32) -> Self {
        Self::sound_of_plugin(id, source_id, 0x0010_0001)
    }

    fn sound_of_plugin(id: u32, source_id: u32, plugin_id: u32) -> Self {
        let mut body = Vec::new();
        put_u32(&mut body, plugin_id);
        body.push(0);
        put_u32(&mut body, source_id);
        body.extend_from_slice(&[0; 9]);
        if plugin_id & PLUGIN_TYPE_MASK == SOURCE_PLUGIN_TYPE {
            put_u32(&mut body, 0);
        }
        body.extend_from_slice(&node_base(TestNodeBase::Plain));
        Self::raw(2, id, body)
    }
}

pub(crate) fn node_base(base: TestNodeBase) -> Vec<u8> {
    let full = base == TestNodeBase::Full;
    let mut body = Vec::new();
    if full {
        body.extend_from_slice(&[1, 1, 0, 0]);
        put_u32(&mut body, 0x3ECB_429B);
        body.extend_from_slice(&[0, 1, 1, 2]);
        put_u32(&mut body, 0x0123_4567);
        body.push(0);
    } else {
        body.extend_from_slice(&[0, 0, 0, 0]);
    }
    put_u32(&mut body, 0);
    put_u32(&mut body, PARENT_ID);
    body.push(0);
    if full {
        body.extend_from_slice(&[1, 0]);
        body.extend_from_slice(&3.0_f32.to_le_bytes());
        body.extend_from_slice(&[1, 2]);
        body.extend_from_slice(&[0; 8]);
        body.extend_from_slice(&[0b0010_0011, 0x01, 1]);
        body.extend_from_slice(&1000_i32.to_le_bytes());
        put_u32(&mut body, 1);
        body.extend_from_slice(&[0; 16]);
        put_u32(&mut body, 1);
        body.extend_from_slice(&[0; 8]);
        body.extend_from_slice(&[0; 12]);
        body.push(0x0C);
        for aux in 1..=4 {
            put_u32(&mut body, 0x6300_0000 + aux);
        }
    } else {
        body.extend_from_slice(&[0, 0, 0, 0]);
    }
    put_u32(&mut body, 0);
    body.extend_from_slice(&[0, 1, 0, 0, 3, 0x0C]);
    if full {
        body.extend_from_slice(&[1, 0, 2, 1]);
        body.extend_from_slice(&variable_length(1));
        put_u32(&mut body, 0xF642_8CC5);
        body.push(0);
        body.extend_from_slice(&variable_length(1));
        put_u32(&mut body, 0x33B9_B4F0);
        put_u16(&mut body, 1);
        put_u16(&mut body, 0);
        body.extend_from_slice(&96.0_f32.to_le_bytes());
        put_u16(&mut body, 1);
        put_u32(&mut body, 0x984F_8A08);
        body.extend_from_slice(&[0, 2]);
        body.extend_from_slice(&variable_length(0));
        put_u32(&mut body, 0x3E18_0FB3);
        body.push(2);
        put_u16(&mut body, 2);
        body.extend_from_slice(&[0; 24]);
    } else {
        body.extend_from_slice(&[0, 0, 0, 0]);
    }
    body
}

fn random_sequence(body: &mut Vec<u8>, sequence: bool, children: &[u32]) {
    put_u16(body, 1);
    put_u16(body, 0);
    put_u16(body, 0);
    body.extend_from_slice(&1000.0_f32.to_le_bytes());
    body.extend_from_slice(&[0; 8]);
    put_u16(body, 1);
    body.extend_from_slice(&[0, 0, u8::from(sequence), 0x12]);
    put_children(body, children);
    put_u16(body, u16::try_from(children.len()).unwrap_or(u16::MAX));
    for child in children {
        put_u32(body, *child);
        put_u32(body, PLAYLIST_WEIGHT);
    }
}

fn switch(body: &mut Vec<u8>, children: &[u32]) {
    body.push(1);
    put_u32(body, 0x7C66_760A);
    put_u32(body, 0x3779_8A64);
    body.push(0);
    put_children(body, children);
    put_u32(body, len_u32(children));
    for (switch, child) in (0x5000_u32..).zip(children) {
        put_u32(body, switch);
        put_u32(body, 1);
        put_u32(body, *child);
    }
    put_u32(body, len_u32(children));
    for child in children {
        put_u32(body, *child);
        body.push(0);
        body.extend_from_slice(&[0; 8]);
    }
}

fn blend(body: &mut Vec<u8>, children: &[u32]) {
    put_children(body, children);
    put_u32(body, 1);
    put_u32(body, 0x0B1E_0001);
    put_u16(body, 0);
    put_u32(body, 0x0B1E_0002);
    body.push(0);
    put_u32(body, len_u32(children));
    for child in children {
        put_u32(body, *child);
        put_u32(body, 1);
        body.extend_from_slice(&[0; 12]);
    }
    body.push(0);
}

fn put_children(body: &mut Vec<u8>, children: &[u32]) {
    put_u32(body, len_u32(children));
    for child in children {
        put_u32(body, *child);
    }
}

fn len_u32<T>(items: &[T]) -> u32 {
    u32::try_from(items.len()).unwrap_or(u32::MAX)
}
