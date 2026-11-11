// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions, Soundbank};
use p4kx_media::MediaRecord;
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject, TestNodeBase, test_key};
use serde_json::json;

use super::audio_bank_state_provider::{source_span, verify_layout};
use super::support::audio_provider::{Origins, Owner, TestResult};

pub(super) const PATH: &str = "node-properties.bnk";
pub(super) const PARENT_ID: u32 = 0x2C21_1B2E;
pub(super) const OVERRIDE_BUS_ID: u32 = 0x7812_1081;
pub(super) const IDS: [u8; 3] = [0xF1, 0x03, 0xF1];
pub(super) const BITS: [u32; 3] = [0x7FC1_2345, 0, 0x8000_0000];
pub(super) const RANGE_IDS: [u8; 2] = [0x03, 0xFA];
pub(super) const RANGE_BITS: [u32; 4] = [0, 0xFF80_0000, 0x0100_0001, 0x7FC0_0019];
const OBJECT_ID: u32 = 0x251;
const NODE_LISTS_OFFSET: usize = 13;
const SOUND_SOURCE_HEADER_LEN: usize = 18;

#[derive(Clone, Copy)]
pub(super) enum Node {
    Sound,
    ActorMixer,
}

impl Node {
    const fn prefix_len(self) -> usize {
        match self {
            Self::Sound => SOUND_SOURCE_HEADER_LEN,
            Self::ActorMixer => 0,
        }
    }

    fn original(self) -> TestHircObject {
        match self {
            Self::Sound => TestHircObject::sound(OBJECT_ID, 123, 2, 0),
            Self::ActorMixer => TestHircObject::actor_mixer(OBJECT_ID, &[], TestNodeBase::Plain),
        }
    }

    const fn type_code(self) -> u8 {
        match self {
            Self::Sound => 2,
            Self::ActorMixer => 7,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Lists<'a> {
    Complete,
    Truncated(&'a [u8]),
    Absent,
}

pub(super) fn bank(node: Node, version: u32, bus: u32, lists: Lists<'_>) -> TestResult<Vec<u8>> {
    let original = TestBank::new(5).object(node.original()).bytes();
    verify_layout(&original, p4kx_extraction::HircLayout::Decoded)?;
    let parsed = Soundbank::parse(&original)?;
    let object = parsed.objects().first().ok_or("original bank object")?;
    let source = parsed
        .object_body(object)
        .get(4..)
        .ok_or("body after object ID")?;
    let lists_start = node.prefix_len() + NODE_LISTS_OFFSET;
    let body = match lists {
        Lists::Absent => source
            .get(..node.prefix_len())
            .ok_or("original Sound source header")?
            .to_vec(),
        Lists::Truncated(values) => {
            let mut body = prefix(source, node, bus)?;
            body.extend(values);
            body
        }
        Lists::Complete => {
            let mut body = prefix(source, node, bus)?;
            body.extend(bundle(&IDS, &BITS)?);
            body.extend(bundle(&RANGE_IDS, &RANGE_BITS)?);
            body.extend(
                source
                    .get(lists_start + 2..)
                    .ok_or("original suffix after two empty counts")?,
            );
            body
        }
    };
    Ok(TestBank::new(5)
        .version(version)
        .object(TestHircObject::raw(node.type_code(), OBJECT_ID, body))
        .object(TestHircObject::event(1, &[]))
        .bytes())
}

pub(super) fn truncate_after_lists(original: &[u8]) -> TestResult<Vec<u8>> {
    let parsed = Soundbank::parse(original)?;
    let node = parsed.objects().first().ok_or("original ActorMixer")?;
    let body = parsed.object_body(node);
    let cut = body
        .get(4..body.len() - 1)
        .ok_or("partial trailing child count")?;
    Ok(TestBank::new(5)
        .object(TestHircObject::raw(7, OBJECT_ID, cut))
        .object(TestHircObject::event(1, &[]))
        .bytes())
}

fn prefix(source: &[u8], node: Node, bus: u32) -> TestResult<Vec<u8>> {
    let mut body = source
        .get(..node.prefix_len() + NODE_LISTS_OFFSET)
        .ok_or("original node prefix")?
        .to_vec();
    let start = node.prefix_len() + 4;
    body.get_mut(start..start + 4)
        .ok_or("original override bus word")?
        .copy_from_slice(&bus.to_le_bytes());
    Ok(body)
}

pub(super) fn bundle(ids: &[u8], words: &[u32]) -> TestResult<Vec<u8>> {
    let mut bytes = vec![u8::try_from(ids.len())?];
    bytes.extend(ids);
    bytes.extend(words.iter().copied().flat_map(u32::to_le_bytes));
    Ok(bytes)
}

pub(super) fn archive(folder: &Path, bytes: &[u8], encrypted: bool) -> TestResult<Archive> {
    let path = folder.join("Data.p4k");
    let leaf = TestEntry::zstd(PATH, bytes);
    let leaf = if encrypted { leaf.encrypted() } else { leaf };
    ArchiveBuilder::new().entry(leaf).write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

pub(super) struct Evidence<'a> {
    source: &'a [u8],
    origins: Origins,
    owner: Owner,
}

impl<'a> Evidence<'a> {
    pub(super) fn read(base: &Path, source: &'a [u8], encrypted: bool) -> TestResult<Self> {
        Ok(Self {
            source,
            origins: Origins::read(base)?,
            owner: Owner {
                path: PATH,
                entry: 0,
                parent: None,
                stored: if encrypted {
                    "encrypted"
                } else {
                    "unencrypted"
                },
                metadata: false,
                payload: encrypted,
            },
        })
    }

    pub(super) fn range(
        &self,
        (position, record): (usize, &MediaRecord),
        field: &str,
        expected: &[u8],
    ) -> TestResult {
        let path = format!("/payload/{field}");
        assert_eq!(
            source_span(self.source, &record.data, &path)?,
            expected,
            "exact {field} bytes"
        );
        let span = record.data.pointer(&path).ok_or("original span")?;
        let indices = json!([span.get("start"), span.get("end_exclusive")]);
        for edge in ["start", "end_exclusive"] {
            let receipt = self.origins.field(
                "records",
                position,
                &format!("/data{path}/{edge}"),
                self.owner.payload,
            )?;
            let uses = self.origins.require_owner(receipt, self.owner)?;
            assert!(
                uses.iter().any(|used| {
                    used.get("role") == Some(&json!("selector"))
                        && used.pointer("/location/field") == Some(&json!("nodePropertyExtent"))
                        && used.pointer("/location/indices") == Some(&indices)
                }),
                "{field} {edge} selects the checked original extent"
            );
        }
        Ok(())
    }

    pub(super) fn word(
        &self,
        (position, record): (usize, &MediaRecord),
        field: &str,
        original: &str,
        value: u32,
        bytes: &[u8],
    ) -> TestResult {
        assert_eq!(
            record.data.pointer(&format!("/payload/{field}/value")),
            Some(&json!(value)),
            "raw {field} value"
        );
        let range = format!("/payload/{field}/range");
        assert_eq!(
            source_span(self.source, &record.data, &range)?,
            bytes,
            "original {field} bytes"
        );
        let span = record.data.pointer(&range).ok_or("original span")?;
        let indices = json!([span.get("start"), span.get("end_exclusive")]);
        let datum = self.origins.field(
            "records",
            position,
            &format!("/data/payload/{field}/value"),
            self.owner.payload,
        )?;
        let uses = self.origins.require_owner(datum, self.owner)?;
        assert!(
            uses.iter().any(|used| {
                used.get("role") == Some(&json!("payload"))
                    && used.pointer("/location/field") == Some(&json!(original))
                    && used.pointer("/location/indices") == Some(&indices)
            }),
            "{field} has the actual physical word receipt from its owning bank"
        );
        Ok(())
    }
}
