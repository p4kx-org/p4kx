// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::PathBuf;

use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestCodec, TestContainerKind, TestEntry, TestHircObject,
    TestNodeBase, TestWem, test_signal, wwise_short_id,
};

use super::super::{EMBEDDED, PLAY, STREAMED, hirc_payload, pcm};
use super::EVENT;

pub(super) const BANK: &str = "Data/Sounds/wwise/Loops.bnk";
pub(super) const LOOSE: &str = "Data/Sounds/wwise/Media/402.wem";
pub(super) const EMBEDDED_WEM: u32 = 401;
pub(super) const LOOSE_WEM: u32 = 402;
const LOOP_CONTAINER: u32 = 10;
const FIRST_SOUND: u32 = 20;
const SECOND_SOUND: u32 = 21;
const DIDX_TRIPLET_BYTES: usize = 3 * size_of::<u32>();
const CHUNK_HEADER_BYTES: usize = 8;
const PLAIN_NODE_LEN: usize = 31;

pub(super) struct Fixture {
    _directory: tempfile::TempDir,
    pub(super) archive: PathBuf,
    pub(super) alternate: PathBuf,
    pub(super) target: PathBuf,
    pub(super) embedded_smpl: usize,
    pub(super) loose_smpl: usize,
    pub(super) loose_data: usize,
    pub(super) didx_triplet: usize,
    pub(super) bank_data: usize,
}

pub(super) fn create() -> Result<Fixture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let alternate = directory.path().join("Alternate.p4k");
    let target = directory.path().join("out");
    let embedded = TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(224, 1))
        .smpl_loop(1, 2, 3)
        .smpl_loop(0, 12, 63)
        .bytes();
    let loose = TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(192, 1))
        .smpl_loop(0, 9, 0)
        .bytes();
    let bank = bank(&embedded)?;
    let definitions = TestAtl::new("Loops").trigger(EVENT, &[EVENT]).to_cryxml();
    for (destination, plain, protected) in [
        (&archive, (LOOSE, loose.as_slice()), (BANK, bank.as_slice())),
        (
            &alternate,
            (BANK, bank.as_slice()),
            (LOOSE, loose.as_slice()),
        ),
    ] {
        ArchiveBuilder::new()
            .entry(TestEntry::zstd(
                "Data/Libs/GameAudio/Loops.xml",
                definitions.as_slice(),
            ))
            .entry(TestEntry::stored(plain.0, plain.1))
            .entry(TestEntry::zstd(protected.0, protected.1).encrypted())
            .write_to(destination)?;
    }
    Ok(Fixture {
        _directory: directory,
        archive,
        alternate,
        target,
        embedded_smpl: body_offset(&embedded, *b"smpl")?,
        loose_smpl: body_offset(&loose, *b"smpl")?,
        loose_data: body_offset(&loose, *b"data")?,
        didx_triplet: body_offset(&bank, *b"DIDX")? + DIDX_TRIPLET_BYTES,
        bank_data: body_offset(&bank, *b"DATA")?,
    })
}

fn bank(embedded: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(TestBank::new(wwise_short_id("Loops"))
        .version(154)
        .wem(399, pcm())
        .wem(EMBEDDED_WEM, embedded)
        .object(TestHircObject::event(wwise_short_id(EVENT), &[1, 2]))
        .object(TestHircObject::action(1, PLAY, LOOP_CONTAINER))
        .object(TestHircObject::action(2, PLAY, SECOND_SOUND))
        .object(loop_container()?)
        .object(TestHircObject::sound(
            FIRST_SOUND,
            EMBEDDED_WEM,
            EMBEDDED,
            0,
        ))
        .object(TestHircObject::sound(SECOND_SOUND, LOOSE_WEM, STREAMED, 0))
        .bytes())
}

fn loop_container() -> Result<TestHircObject, Box<dyn Error>> {
    let object = TestHircObject::container(
        TestContainerKind::Sequence,
        LOOP_CONTAINER,
        &[FIRST_SOUND],
        TestNodeBase::Plain,
    );
    let mut payload = hirc_payload(object)?;
    payload
        .get_mut(PLAIN_NODE_LEN..PLAIN_NODE_LEN + size_of::<u16>())
        .ok_or("authored container loop count")?
        .copy_from_slice(&3_u16.to_le_bytes());
    Ok(TestHircObject::raw(5, LOOP_CONTAINER, payload))
}

fn body_offset(bytes: &[u8], tag: [u8; 4]) -> Result<usize, Box<dyn Error>> {
    bytes
        .windows(tag.len())
        .position(|window| window == tag.as_slice())
        .map(|offset| offset + CHUNK_HEADER_BYTES)
        .ok_or_else(|| format!("synthetic chunk {}", String::from_utf8_lossy(&tag)).into())
}
