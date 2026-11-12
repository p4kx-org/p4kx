// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The harness of the TXTP tests: a synthetic Archive with one trigger, exported and written as
//! TXTP.

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_assembly::audio::{ArchiveAudio, ExportOptions, SoundFormat, TxtpReport, write_txtp};
use p4kx_extraction::{Archive, ConflictPolicy};
use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestCodec, TestContainerKind, TestEntry, TestHircObject,
    TestNodeBase, TestWem, test_signal, wwise_short_id,
};

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

pub(crate) const PLAY: u16 = 0x0403;
pub(crate) const EMBEDDED: u8 = 0;
pub(crate) const PREFETCH: u8 = 1;
const FRAMES: u32 = 480;

/// A one-channel PCM WEM.
pub(crate) fn pcm() -> TestWem {
    TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(FRAMES, 1))
}

pub(crate) fn sound(id: u32, wem: u32, storage: u8, size: usize) -> TestHircObject {
    TestHircObject::sound(id, wem, storage, u32::try_from(size).unwrap_or(0))
}

pub(crate) fn container(kind: TestContainerKind, id: u32, children: &[u32]) -> TestHircObject {
    TestHircObject::container(kind, id, children, TestNodeBase::Plain)
}

/// The Event object `Play_X` with the action 1.
pub(crate) fn event() -> TestHircObject {
    TestHircObject::event(wwise_short_id("Play_X"), &[1])
}

pub(crate) fn read(folder: &Path, name: &str) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(folder.join(name))?)
}

/// Writes the TXTPs again under a Conflict policy.
pub(crate) type Rerun<'a> = dyn Fn(ConflictPolicy) -> TxtpReport + 'a;

/// What the archive of a test holds and how its sounds are exported.
#[derive(Default)]
pub(crate) struct Case {
    /// The audio trigger; `Play_X` when `None`.
    pub(crate) trigger: Option<String>,
    /// The Wwise events the trigger posts; `Play_X` when empty.
    pub(crate) events: Vec<String>,
    /// The format of the sounds; WAV when `None`.
    pub(crate) format: Option<SoundFormat>,
    /// A folder below the export folder, to make paths long.
    pub(crate) folder: Option<String>,
    /// Whether the TXTPs name the files of a second export under the Skip policy.
    pub(crate) reexport: bool,
    /// WEMs that are Entries of their own.
    pub(crate) loose: Vec<(u32, Vec<u8>)>,
    /// WEMs embedded in the soundbank.
    pub(crate) wems: Vec<(u32, Vec<u8>)>,
    pub(crate) objects: Vec<TestHircObject>,
    /// Additional physical bank Entries, in Archive order.
    pub(crate) additional_banks: Vec<(String, Vec<u8>)>,
}

impl Case {
    /// Resolves `Play_X`, exports its sounds, writes the TXTPs and hands the export folder, the
    /// report and a way to write again to `check`.
    pub(crate) fn run(
        self,
        check: impl FnOnce(&Path, &TxtpReport, &Rerun<'_>) -> TestResult,
    ) -> TestResult {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("Data.p4k");
        let mut bank = TestBank::new(wwise_short_id("Bank"));
        for (id, wem) in &self.wems {
            bank = bank.wem(*id, wem.clone());
        }
        for object in self.objects {
            bank = bank.object(object);
        }
        let trigger = self.trigger.as_deref().unwrap_or("Play_X");
        let events = if self.events.is_empty() {
            vec!["Play_X".to_owned()]
        } else {
            self.events
        };
        let names: Vec<&str> = events.iter().map(String::as_str).collect();
        let builder = ArchiveBuilder::new()
            .entry(TestEntry::zstd(
                "Data\\Libs\\GameAudio\\SC.xml",
                TestAtl::new("SC").trigger(trigger, &names).to_cryxml(),
            ))
            .entry(TestEntry::stored(
                "Data\\Sounds\\wwise\\Bank.bnk",
                bank.bytes(),
            ));
        additional_entries(builder, self.additional_banks, self.loose).write_to(&path)?;
        let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
        let audio = ArchiveAudio::new(&archive);
        let sounds = audio.resolve_trigger(trigger).ok_or("resolves")?;
        let out = tempfile::tempdir()?;
        let folder = self
            .folder
            .map_or_else(|| out.path().to_path_buf(), |name| out.path().join(name));
        let options = ExportOptions {
            format: self.format.unwrap_or(SoundFormat::Wav),
            policy: ConflictPolicy::Overwrite,
        };
        let triggers = [&sounds];
        let first = audio.export_sounds(&triggers, &folder, options)?;
        let exported = if self.reexport {
            let skip = ExportOptions {
                policy: ConflictPolicy::Skip,
                ..options
            };
            audio.export_sounds(&triggers, &folder, skip)?
        } else {
            first
        };
        let write = |policy| write_txtp(&triggers, &exported, &folder, policy);
        let report = write(ConflictPolicy::Overwrite);
        check(&folder, &report, &write)
    }
}

fn additional_entries(
    mut builder: ArchiveBuilder,
    banks: Vec<(String, Vec<u8>)>,
    loose: Vec<(u32, Vec<u8>)>,
) -> ArchiveBuilder {
    for (name, bytes) in banks {
        builder = builder.entry(TestEntry::stored(
            format!("Data/Sounds/wwise/{name}"),
            bytes,
        ));
    }
    for (id, bytes) in loose {
        builder = builder.entry(TestEntry::stored(
            format!("Data\\Sounds\\wwise\\Media\\{id}.wem"),
            bytes,
        ));
    }
    builder
}

/// A case with the WEMs embedded in the soundbank.
pub(crate) fn embedded(wems: Vec<(u32, Vec<u8>)>, objects: Vec<TestHircObject>) -> Case {
    Case {
        wems,
        objects,
        ..Case::default()
    }
}
