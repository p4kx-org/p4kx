// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_archive::Archive;
use p4kx_audio::{BankError, Soundbank};
use p4kx_testkit::{ArchiveBuilder, TestAtl, TestEntry};
use tempfile::TempDir;

use crate::audio::{
    AudioFileError, AudioIndex, IndexedSoundbank, MAX_ATL_LEN, MAX_SOUNDBANK_LEN, WwiseEvent,
};

use super::super::Loaded;

#[path = "../../tests/support/audio_bank_graph.rs"]
mod graph;
#[path = "../../tests/it/audio_bank_range_fixture.rs"]
mod ranges;

use graph::{Bank, EVENT, Object, PLAY, POST, TRIGGER};
pub(super) use ranges::Case;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const ATL_PATH: &str = "Data\\Libs\\GameAudio\\original-private-bank-probe.xml";

pub(super) struct Fixture {
    folder: TempDir,
    pub(super) archive: Archive,
    banks: Vec<Bank>,
}

pub(super) fn with_source(
    case: Case,
    check: impl FnOnce(&Fixture, &AudioIndex) -> TestResult,
) -> TestResult {
    let banks = ranges::banks(case);
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("bank-probe.p4k");
    write_source(&path, &banks, None)?;
    let source = Fixture {
        folder,
        archive: Archive::open(path)?,
        banks,
    };
    source.assert_stored(None)?;
    let index = AudioIndex::build(&source.archive);
    assert!(
        index.issues().is_empty(),
        "all original inputs parse before the worker probe"
    );
    let names: Vec<_> = index
        .soundbanks()
        .iter()
        .map(IndexedSoundbank::file_name)
        .collect();
    assert_eq!(
        names,
        ["A-second.bnk", "M-first.bnk", "Z-home.bnk"],
        "original index order"
    );
    let homes: Vec<_> = index
        .event_soundbanks(graph::root_event())
        .map(IndexedSoundbank::file_name)
        .collect();
    assert_eq!(homes, ["Z-home.bnk"], "one original home");
    check(&source, &index)
}

pub(super) fn position(index: &AudioIndex, name: &str) -> TestResult<u32> {
    let at = index
        .soundbanks()
        .iter()
        .position(|bank| bank.file_name() == name)
        .ok_or("original indexed bank")?;
    Ok(u32::try_from(at)?)
}

pub(super) fn event(index: &AudioIndex) -> TestResult<&WwiseEvent> {
    index
        .trigger(TRIGGER)
        .and_then(|trigger| trigger.events().first())
        .ok_or_else(|| "actual original Event".into())
}

impl Fixture {
    fn assert_stored(&self, corrupt: Option<&str>) -> TestResult {
        self.assert_bytes(ATL_PATH, original_atl().as_bytes(), MAX_ATL_LEN)?;
        for bank in &self.banks {
            self.assert_bank(bank, corrupt)?;
        }
        Ok(())
    }

    fn assert_bank(&self, bank: &Bank, corrupt: Option<&str>) -> TestResult {
        let expected = stored_bytes(bank, corrupt)?;
        let actual = self.assert_bytes(&bank.path, &expected, MAX_SOUNDBANK_LEN)?;
        if is_selected(bank, corrupt) {
            assert_eq!(
                Soundbank::parse(actual.as_slice()).err(),
                Some(BankError::NotSoundbank),
                "actual malformed original header"
            );
        } else {
            assert_readable(&actual, bank)?;
        }
        Ok(())
    }

    fn assert_bytes(&self, path: &str, expected: &[u8], limit: u64) -> TestResult<Vec<u8>> {
        use std::io::Read as _;
        let entry = self
            .archive
            .entries()
            .iter()
            .find(|entry| entry.path() == path)
            .ok_or("actual source Entry")?;
        assert_eq!(
            entry.uncompressed_size(),
            u64::try_from(expected.len())?,
            "original complete size"
        );
        assert!(
            entry.uncompressed_size() < limit,
            "independent source-byte admission"
        );
        let mut actual = Vec::new();
        self.archive.open_entry(entry)?.read_to_end(&mut actual)?;
        assert_eq!(
            actual, expected,
            "the actual Archive exposes the exact current fixture bytes"
        );
        Ok(actual)
    }

    fn rewrite(&self, corrupt: Option<&str>) -> TestResult {
        let path = self.folder.path().join("bank-probe.p4k");
        let before = fs::metadata(&path)?.len();
        write_source(&path, &self.banks, corrupt)?;
        assert_eq!(
            fs::metadata(path)?.len(),
            before,
            "stored Entry offsets/sizes stay unchanged"
        );
        self.assert_stored(corrupt)
    }

    pub(super) fn corrupt_first(&self, index: &AudioIndex) -> TestResult<AudioFileError> {
        self.rewrite(Some("M-first.bnk"))?;
        let Err(error) = Loaded::read(&self.archive, index, position(index, "M-first.bnk")?) else {
            return Err("actual original corruption did not fail".into());
        };
        assert_eq!(
            error,
            AudioFileError::Soundbank(BankError::NotSoundbank),
            "actual worker loader/parser error before counting"
        );
        Ok(error)
    }

    pub(super) fn restore_first(&self, index: &AudioIndex) -> TestResult {
        self.rewrite(None)?;
        let loaded = Loaded::read(&self.archive, index, position(index, "M-first.bnk")?)?;
        assert_eq!(
            loaded.bank.header().id,
            loaded.soundbank.id(),
            "real direct loader succeeds after source recovery"
        );
        Ok(())
    }
}

fn assert_readable(bytes: &[u8], original: &Bank) -> TestResult {
    let parsed = Soundbank::parse(bytes)?;
    assert_eq!(parsed.header().id, original.id, "original bank header");
    assert_eq!(parsed.header().version, 154, "original supported layout");
    assert!(
        parsed.warnings().is_empty(),
        "healthy fixture has no layout warnings"
    );
    let ids: Vec<_> = parsed.objects().iter().map(|object| object.id).collect();
    let expected: Vec<_> = original.objects.iter().map(Object::id).collect();
    assert_eq!(
        ids, expected,
        "every original object occurrence in source order"
    );
    Ok(())
}

fn is_selected(bank: &Bank, corrupt: Option<&str>) -> bool {
    corrupt.is_some_and(|name| bank.path.rsplit('\\').next() == Some(name))
}

fn stored_bytes(bank: &Bank, corrupt: Option<&str>) -> TestResult<Vec<u8>> {
    let mut bytes = bank.bytes.clone();
    if is_selected(bank, corrupt) {
        let magic = bytes.get_mut(..4).ok_or("original complete bank header")?;
        assert_eq!(
            magic, b"BKHD",
            "the known fixture header is changed, never a searched archive pattern"
        );
        magic.copy_from_slice(b"XXXX");
    }
    Ok(bytes)
}

fn write_source(path: &Path, banks: &[Bank], corrupt: Option<&str>) -> TestResult {
    let atl = original_atl();
    let mut builder = ArchiveBuilder::new().entry(TestEntry::stored(ATL_PATH, atl.as_bytes()));
    for bank in banks {
        builder = builder.entry(TestEntry::stored(
            bank.path.as_str(),
            stored_bytes(bank, corrupt)?,
        ));
    }
    builder.write_to(path)?;
    Ok(())
}

fn original_atl() -> String {
    TestAtl::new("OriginalPrivateBankProbe")
        .trigger(TRIGGER, &[EVENT])
        .to_xml()
}
