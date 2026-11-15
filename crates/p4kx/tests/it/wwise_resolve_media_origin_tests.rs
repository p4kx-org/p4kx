// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestBank, TestEntry, TestHircObject, wwise_short_id};
use serde_json::json;

use super::{PLAY, Proof, TestResult, object_bytes, probe};

const CIPHER_BANK: &str = "Data\\Sounds\\wwise\\Cobalt.bnk";
const PLAIN_BANK: &str = "Data\\Sounds\\wwise\\Quartz.bnk";
const EMBEDDED_BANK: &str = "Data\\Sounds\\wwise\\PackedMedia.bnk";
const LOOSE_WEM: &str = "Data\\Sounds\\wwise\\Media\\5000.wem";
const TRIGGER: &str = "ProbeMedia";
const SEALED_EVENT: &str = "CobaltEvent";
const OPEN_EVENT: &str = "QuartzEvent";
const STREAM_ID: u32 = 5000;
const EMBED_ID: u32 = 5001;
const MIDI_ID: u32 = 5002;
const STREAM_SOUND: u32 = 10;
const TRACK: u32 = 11;
const EMBED_SOUND: u32 = 20;
const PLUGIN_SOUND: u32 = 21;
const EXTERNAL_SOUND: u32 = 22;
const MIDI_SOUND: u32 = 23;
const ID_BYTES: usize = size_of::<u32>();
const TYPE_BYTES: usize = size_of::<u8>();
const HIRC_HEADER_BYTES: usize = TYPE_BYTES + 2 * ID_BYTES;
const V154_TRACK_SOURCE_BYTES: usize = 4 * ID_BYTES + 2 * TYPE_BYTES;

#[test]
fn wwise_terminal_fields_cite_the_declaring_bank_and_each_physical_source_row() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    let sealed = sealed_bank();
    let ordinary = ordinary_bank();
    write_fixture(&path, &sealed, &ordinary)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let result = probe(&path, TRIGGER)?;
    let proof = Proof {
        result: &result,
        archive: &archive,
    };
    assert_eq!(
        result.pointer("/counts/variants"),
        Some(&json!(7)),
        "all authored source rows reach the result"
    );
    assert_eq!(
        result.pointer("/variants/0/location/entry"),
        Some(&json!(LOOSE_WEM)),
        "the encrypted declaration selects a plain loose WEM"
    );
    assert_eq!(
        result.pointer("/variants/3/location/soundbank"),
        Some(&json!(EMBEDDED_BANK)),
        "the plain declaration selects WEM bytes from an encrypted bank"
    );
    verify_encrypted(&proof, &sealed)?;
    verify_plain(&proof, &ordinary)
}

fn sealed_bank() -> Vec<u8> {
    TestBank::new(wwise_short_id("Cobalt"))
        .object(TestHircObject::event(wwise_short_id(SEALED_EVENT), &[1, 2]))
        .object(TestHircObject::action(1, PLAY, STREAM_SOUND))
        .object(TestHircObject::action(2, PLAY, TRACK))
        .object(TestHircObject::sound(STREAM_SOUND, STREAM_ID, 2, 0))
        .object(TestHircObject::music_track(TRACK, &[STREAM_ID, STREAM_ID]))
        .bytes()
}

fn ordinary_bank() -> Vec<u8> {
    TestBank::new(wwise_short_id("Quartz"))
        .object(TestHircObject::event(
            wwise_short_id(OPEN_EVENT),
            &[1, 2, 3, 4],
        ))
        .object(TestHircObject::action(1, PLAY, EMBED_SOUND))
        .object(TestHircObject::action(2, PLAY, PLUGIN_SOUND))
        .object(TestHircObject::action(3, PLAY, EXTERNAL_SOUND))
        .object(TestHircObject::action(4, PLAY, MIDI_SOUND))
        .object(TestHircObject::sound(EMBED_SOUND, EMBED_ID, 0, 8))
        .object(TestHircObject::plugin_sound(PLUGIN_SOUND, 6000))
        .object(TestHircObject::external_sound(EXTERNAL_SOUND, 6001))
        .object(TestHircObject::midi_sound(MIDI_SOUND, MIDI_ID))
        .bytes()
}

fn write_fixture(path: &Path, sealed: &[u8], ordinary: &[u8]) -> TestResult {
    let atl = TestAtl::new("SC_Proof").trigger(TRIGGER, &[SEALED_EVENT, OPEN_EVENT]);
    let packed = TestBank::new(wwise_short_id("PackedMedia"))
        .wem(EMBED_ID, vec![0x5A; 8])
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC_Proof.xml",
            atl.to_cryxml(),
        ))
        .entry(TestEntry::zstd(CIPHER_BANK, sealed).encrypted())
        .entry(TestEntry::stored(PLAIN_BANK, ordinary))
        .entry(TestEntry::zstd(EMBEDDED_BANK, packed).encrypted())
        .entry(TestEntry::stored(LOOSE_WEM, vec![0xD5; 8]))
        .write_to(path)?;
    Ok(())
}

struct Bank<'a> {
    path: &'static str,
    bytes: &'a [u8],
    encrypted: bool,
}

#[derive(Clone, Copy)]
struct Terminal {
    position: usize,
    object: u32,
    kind: &'static str,
    wem: Option<u32>,
    track_source: Option<usize>,
    storage: bool,
}

fn verify_encrypted(proof: &Proof<'_>, bytes: &[u8]) -> TestResult {
    let bank = Bank {
        path: CIPHER_BANK,
        bytes,
        encrypted: true,
    };
    bank.check(
        proof,
        Terminal {
            position: 0,
            object: STREAM_SOUND,
            kind: "streamed",
            wem: Some(STREAM_ID),
            track_source: None,
            storage: true,
        },
    )?;
    for ordinal in 0..2 {
        bank.check(
            proof,
            Terminal {
                position: 1 + ordinal,
                object: TRACK,
                kind: "streamed",
                wem: Some(STREAM_ID),
                track_source: Some(ordinal),
                storage: true,
            },
        )?;
    }
    Ok(())
}

fn verify_plain(proof: &Proof<'_>, bytes: &[u8]) -> TestResult {
    let bank = Bank {
        path: PLAIN_BANK,
        bytes,
        encrypted: false,
    };
    for terminal in [
        Terminal {
            position: 3,
            object: EMBED_SOUND,
            kind: "embedded",
            wem: Some(EMBED_ID),
            track_source: None,
            storage: true,
        },
        Terminal {
            position: 4,
            object: PLUGIN_SOUND,
            kind: "plugin",
            wem: None,
            track_source: None,
            storage: false,
        },
        Terminal {
            position: 5,
            object: EXTERNAL_SOUND,
            kind: "external",
            wem: None,
            track_source: None,
            storage: false,
        },
        Terminal {
            position: 6,
            object: MIDI_SOUND,
            kind: "midi",
            wem: Some(MIDI_ID),
            track_source: None,
            storage: false,
        },
    ] {
        bank.check(proof, terminal)?;
    }
    Ok(())
}

impl Bank<'_> {
    fn check(&self, proof: &Proof<'_>, terminal: Terminal) -> TestResult {
        let base = format!("/variants/{}", terminal.position);
        for (name, expected) in [
            ("wem_id", json!(terminal.wem)),
            ("source", json!(terminal.kind)),
        ] {
            assert_eq!(
                proof.result.pointer(&format!("{base}/{name}")),
                Some(&expected),
                "{name} keeps its declared resolver value"
            );
        }
        let start = object_bytes(self.bytes, terminal.object)?.start;
        let fields = Row::of(start, terminal.track_source);
        let common = fields.selectors(start);
        let plugin_end = fields.start + ID_BYTES;
        let storage_end = plugin_end + TYPE_BYTES;
        let media_id = terminal
            .wem
            .map(|_| fields.site("sourceId", "payload", storage_end..storage_end + ID_BYTES));
        let storage = terminal
            .storage
            .then(|| fields.site("storageCode", "conversion", plugin_end..storage_end));
        self.check_field(proof, &format!("{base}/wem_id"), &common, media_id.as_ref())?;
        self.check_field(proof, &format!("{base}/source"), &common, storage.as_ref())
    }

    fn check_field(
        &self,
        proof: &Proof<'_>,
        address: &str,
        common: &[Site],
        selected: Option<&Site>,
    ) -> TestResult {
        let origin = proof.origin(address)?;
        assert_eq!(
            origin["containsEncryptedSource"], self.encrypted,
            "{address} is marked from its declaration"
        );
        assert_eq!(
            origin["sourceSetComplete"], true,
            "{address} is fully evidenced"
        );
        assert_eq!(
            origin["sourceUsesRefs"].as_array().map(Vec::len),
            Some(common.len() + usize::from(selected.is_some())),
            "{address} references exactly its declaration sites"
        );
        for site in common.iter().chain(selected) {
            let actual = proof.site_at(
                origin,
                self.path,
                site.namespace,
                site.name,
                &site.indices(),
            )?;
            assert_eq!(
                actual["role"], site.role,
                "{address} uses {} as {}",
                site.name, site.role
            );
        }
        Ok(())
    }
}

struct Row {
    start: usize,
    track_source: Option<usize>,
}

impl Row {
    fn of(object: usize, track_source: Option<usize>) -> Self {
        let body = object + HIRC_HEADER_BYTES;
        let start = track_source.map_or(body, |ordinal| {
            body + ID_BYTES + ordinal * V154_TRACK_SOURCE_BYTES
        });
        Self {
            start,
            track_source,
        }
    }

    const fn site(&self, name: &'static str, role: &'static str, bytes: Range<usize>) -> Site {
        Site {
            namespace: if self.track_source.is_some() {
                "bnkMusicTrackSource"
            } else {
                "bnkSoundMediaSource"
            },
            name,
            role,
            bytes,
            ordinal: self.track_source,
        }
    }

    fn selectors(&self, object: usize) -> Vec<Site> {
        let mut sites = vec![
            Site::selector("bnkHircObject", "typeCode", object..object + TYPE_BYTES),
            Site::selector("bnkBkhd", "version", 8..12),
            self.site("pluginId", "selector", self.start..self.start + ID_BYTES),
        ];
        if self.track_source.is_some() {
            let body = object + HIRC_HEADER_BYTES;
            sites.push(Site::selector(
                "bnkMusicTrack",
                "sourceCount",
                body..body + ID_BYTES,
            ));
        }
        sites
    }
}

struct Site {
    namespace: &'static str,
    name: &'static str,
    role: &'static str,
    bytes: Range<usize>,
    ordinal: Option<usize>,
}

impl Site {
    const fn selector(namespace: &'static str, name: &'static str, bytes: Range<usize>) -> Self {
        Self {
            namespace,
            name,
            role: "selector",
            bytes,
            ordinal: None,
        }
    }

    fn indices(&self) -> Vec<String> {
        let mut result = vec![self.bytes.start.to_string(), self.bytes.end.to_string()];
        result.extend(self.ordinal.map(|ordinal| ordinal.to_string()));
        result
    }
}
