// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_testkit::{TestBank, TestHircObject};
use serde_json::{Value, json};

pub(super) const PARENT: u32 = 0;
pub(super) const SOURCE_ID: u32 = 0x2000_0201;
const OBJECT_ID: u32 = 0x1000_0101;
const SOUND_TAG: u8 = 2;

pub(super) struct Fixture {
    pub(super) bytes: Vec<u8>,
    pub(super) expected: Vec<Word>,
    pub(super) continuation_bytes: Range<usize>,
}

pub(super) struct Word {
    pub(super) path: &'static str,
    pub(super) name: &'static str,
    pub(super) range: Range<usize>,
    pub(super) value: Value,
}

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Opaque,
    Full,
    Truncated,
    Unsupported,
}

pub(super) fn bank(layout: Layout) -> Fixture {
    let mut body = Body::default();
    match layout {
        Layout::Opaque => {
            body.source(0x0008_0001, 1, false);
            let end = body.bytes.len();
            body.bytes.extend_from_slice(&[0x8e, 0x9f]);
            body.finish(150, end..end + 2)
        }
        Layout::Full => {
            body.source(0x0004_0002, 0xa5, true);
            let raw = body.plugin_block(3);
            body.node();
            body.finish(154, raw)
        }
        Layout::Truncated => {
            body.source(0x0004_0002, 0xa5, true);
            let raw = body.plugin_block(8);
            body.finish(154, raw)
        }
        Layout::Unsupported => body.finish(155, 0..0),
    }
}

#[derive(Default)]
struct Body {
    bytes: Vec<u8>,
    expected: Vec<Word>,
}

impl Body {
    fn source(&mut self, plugin: u32, storage: u8, cache: bool) {
        self.u32("/source/plugin_id", "soundPluginId", plugin);
        self.u8("/source/storage_code", "soundStorage", storage);
        self.u32("/source/source_id", "soundSourceId", SOURCE_ID);
        if cache {
            self.u32("/source/cache_id", "soundCacheId", 0xa0b0_c0d0);
        }
        self.u32("/source/in_memory_size", "soundMemorySize", 702);
        self.u8("/source/source_bits", "soundSourceBits", 0xe1);
    }

    fn plugin_block(&mut self, declared: u32) -> Range<usize> {
        self.u32(
            "/continuation/plugin_parameter_length",
            "soundPluginParameterLength",
            declared,
        );
        let start = self.bytes.len();
        self.bytes.extend_from_slice(&[0xff, 0x00, 0xda]);
        start..self.bytes.len()
    }

    fn node(&mut self) {
        self.bytes.push(1);
        self.u8(
            "/continuation/node_base/effect_count",
            "soundNodeEffectCount",
            1,
        );
        self.bytes.extend_from_slice(&[0, 1, 2, 3, 4, 5, 6]);
        self.bytes.push(0);
        self.u8(
            "/continuation/node_base/metadata_count",
            "soundNodeMetadataCount",
            1,
        );
        self.bytes.extend_from_slice(&[7, 8, 9, 10, 11, 12]);
        self.u32(
            "/continuation/node_base/pre_parent_word",
            "soundNodePreParentWord",
            0xfebc_a987,
        );
        self.u32(
            "/continuation/node_base/parent_id",
            "soundNodeParentId",
            PARENT,
        );
        self.bytes.extend_from_slice(&[0; 19]);
    }

    fn u8(&mut self, path: &'static str, name: &'static str, value: u8) {
        self.word(path, name, &[value], json!(value));
    }

    fn u32(&mut self, path: &'static str, name: &'static str, value: u32) {
        self.word(path, name, &value.to_le_bytes(), json!(value));
    }

    fn word(&mut self, path: &'static str, name: &'static str, bytes: &[u8], value: Value) {
        let start = self.bytes.len();
        self.bytes.extend_from_slice(bytes);
        self.expected.push(Word {
            path,
            name,
            range: start..self.bytes.len(),
            value,
        });
    }

    fn finish(self, version: u32, continuation: Range<usize>) -> Fixture {
        let length = self.bytes.len();
        let bytes = TestBank::new(71)
            .version(version)
            .object(TestHircObject::raw(SOUND_TAG, OBJECT_ID, self.bytes))
            .bytes();
        let base = bytes.len() - length;
        let mut expected = self.expected;
        for word in &mut expected {
            word.range.start += base;
            word.range.end += base;
        }
        Fixture {
            bytes,
            expected,
            continuation_bytes: base + continuation.start..base + continuation.end,
        }
    }
}
