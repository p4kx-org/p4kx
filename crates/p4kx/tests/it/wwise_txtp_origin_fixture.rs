// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::PathBuf;

use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase,
    wwise_short_id,
};

use super::super::{EMBEDDED, PLAY, hirc_payload, pcm};
use super::EVENT;

pub(super) const MAIN_BANK: &str = "Data/Sounds/wwise/Origin.bnk";
pub(super) const BLEND_BANK: &str = "Data/Sounds/wwise/Blend.bnk";
pub(super) const SEQUENCE: u32 = 10;
pub(super) const BLEND: u32 = 11;
const SOUND: u32 = 20;
const BLEND_SOUND: u32 = 21;
const WEM: u32 = 101;
const BLEND_WEM: u32 = 102;
pub(super) const DELAY: u8 = 3;
const PLAIN_NODE_LEN: usize = 31;
pub(super) const MODE_OFFSET: usize = PLAIN_NODE_LEN + 4 * size_of::<u16>() + 3 * size_of::<f32>();

pub(super) struct Fixture {
    _directory: tempfile::TempDir,
    pub(super) archive: PathBuf,
    pub(super) alternate: PathBuf,
    pub(super) out: PathBuf,
    pub(super) main_bank: Vec<u8>,
    pub(super) blend_bank: Vec<u8>,
}

pub(super) fn create() -> Result<Fixture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let alternate = directory.path().join("Alternate.p4k");
    let out = directory.path().join("out");
    let blend_id = wwise_short_id("Blend");
    let main_bank = TestBank::new(wwise_short_id("Origin"))
        .version(154)
        .wem(WEM, pcm())
        .object(TestHircObject::event(wwise_short_id(EVENT), &[1, 2]))
        .object(TestHircObject::action(1, PLAY, SEQUENCE))
        .object(TestHircObject::action_in_bank(2, PLAY, BLEND, blend_id))
        .object(sequence()?)
        .object(TestHircObject::sound(SOUND, WEM, EMBEDDED, 0))
        .bytes();
    let blend_bank = TestBank::new(blend_id)
        .version(154)
        .wem(BLEND_WEM, pcm())
        .object(TestHircObject::container(
            TestContainerKind::Blend,
            BLEND,
            &[BLEND_SOUND],
            TestNodeBase::Plain,
        ))
        .object(TestHircObject::sound(BLEND_SOUND, BLEND_WEM, EMBEDDED, 0))
        .bytes();
    for (path, plain, protected) in [
        (&archive, (MAIN_BANK, &main_bank), (BLEND_BANK, &blend_bank)),
        (
            &alternate,
            (BLEND_BANK, &blend_bank),
            (MAIN_BANK, &main_bank),
        ),
    ] {
        let atl = TestAtl::new("Origin").trigger(EVENT, &[EVENT]).to_cryxml();
        ArchiveBuilder::new()
            .entry(TestEntry::zstd("Data/Libs/GameAudio/Origin.xml", atl))
            .entry(TestEntry::stored(plain.0, plain.1.as_slice()))
            .entry(TestEntry::zstd(protected.0, protected.1.as_slice()).encrypted())
            .write_to(path)?;
    }
    Ok(Fixture {
        _directory: directory,
        archive,
        alternate,
        out,
        main_bank,
        blend_bank,
    })
}

fn sequence() -> Result<TestHircObject, Box<dyn Error>> {
    let object = TestHircObject::container(
        TestContainerKind::Sequence,
        SEQUENCE,
        &[SOUND],
        TestNodeBase::Plain,
    );
    let mut payload = hirc_payload(object)?;
    let loops = payload
        .get_mut(PLAIN_NODE_LEN..PLAIN_NODE_LEN + size_of::<u16>())
        .ok_or("original loop count")?;
    loops.copy_from_slice(&3_u16.to_le_bytes());
    let mode = payload
        .get_mut(MODE_OFFSET)
        .ok_or("original transition mode")?;
    *mode = DELAY;
    Ok(TestHircObject::raw(5, SEQUENCE, payload))
}
