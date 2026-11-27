// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use lewton::inside_ogg::OggStreamReader;
use p4kx_extraction::{
    Archive, AudioConversion, ConflictPolicy, EntryFailure, ExtractOptions, ExtractionReport,
    extract,
};
use p4kx_testkit::{
    ArchiveBuilder, TestBank, TestCodec, TestEntry, TestVorbis, TestWem, test_signal,
};

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

pub(crate) const VORBIS: &str = "Data\\Sounds\\wwise\\Media\\500.wem";
pub(crate) const PCM: &str = "Data\\Sounds\\wwise\\Media\\600.wem";
pub(crate) const XMA: &str = "Data\\Sounds\\wwise\\Media\\700.wem";
pub(crate) const BANK: &str = "Data\\Sounds\\wwise\\Ship.bnk";
pub(crate) const PCM_FRAMES: u32 = 480;
pub(crate) const PREFETCH_LEN: usize = 64;

pub(crate) fn vorbis() -> Vec<u8> {
    TestVorbis::new(1, 24_000, &[false, true, true, false, false])
        .wem()
        .bytes()
}

pub(crate) fn pcm() -> Vec<u8> {
    TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(PCM_FRAMES, 1)).bytes()
}

pub(crate) fn xma() -> Vec<u8> {
    TestWem::encoded(TestCodec::Tag(0x0166), 2, 48_000, 1_000).bytes()
}

pub(crate) fn prefetch() -> Vec<u8> {
    pcm().get(..PREFETCH_LEN).map_or_default(<[u8]>::to_vec)
}

/// A soundbank embedding a Wwise Vorbis, a PCM and an XMA2 WEM and a prefetched start.
pub(crate) fn bank() -> Vec<u8> {
    TestBank::new(1)
        .wem(10, vorbis())
        .wem(20, pcm())
        .wem(30, xma())
        .wem(40, prefetch())
        .bytes()
}

pub(crate) struct Fixture {
    _dir: tempfile::TempDir,
    pub(crate) archive: Archive,
    pub(crate) target: PathBuf,
}

impl Fixture {
    pub(crate) fn new(entries: Vec<TestEntry>) -> Result<Self, Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("Data.p4k");
        entries
            .into_iter()
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .write_to(&path)?;
        let target = dir.path().join("out");
        fs::create_dir(&target)?;
        Ok(Self {
            archive: Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?,
            target,
            _dir: dir,
        })
    }

    /// The loose Vorbis, PCM and XMA2 WEMs and the soundbank.
    pub(crate) fn sounds() -> Result<Self, Box<dyn Error>> {
        Self::new(vec![
            TestEntry::zstd(VORBIS, vorbis()),
            TestEntry::stored(PCM, pcm()),
            TestEntry::zstd(XMA, xma()),
            TestEntry::zstd(BANK, bank()),
        ])
    }

    pub(crate) fn run(&self, audio: AudioConversion, policy: ConflictPolicy) -> ExtractionReport {
        self.run_with(&ExtractOptions {
            policy,
            convert: true,
            audio,
            ..ExtractOptions::default()
        })
    }

    pub(crate) fn run_with(&self, options: &ExtractOptions<'_>) -> ExtractionReport {
        let entries: Vec<_> = self.archive.entries().iter().collect();
        extract(&self.archive, &entries, &self.target, options)
    }

    pub(crate) fn read(&self, path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(fs::read(self.target.join(path.replace('\\', "/")))?)
    }

    pub(crate) fn exists(&self, path: &str) -> bool {
        self.target.join(path.replace('\\', "/")).exists()
    }

    /// Returns the file names in the folder `path` below the target, sorted.
    pub(crate) fn names(&self, path: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let mut names: Vec<String> = fs::read_dir(self.target.join(path.replace('\\', "/")))?
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
            .collect::<Result<_, _>>()?;
        names.sort();
        Ok(names)
    }
}

/// Returns the error of a failure with all its sources.
pub(crate) fn reason(failure: &EntryFailure) -> String {
    let mut reason = failure.error().to_string();
    let mut source = Error::source(failure.error());
    while let Some(cause) = source {
        reason.push_str(": ");
        reason.push_str(&cause.to_string());
        source = cause.source();
    }
    reason
}

/// Decodes an Ogg Vorbis file and returns the number of samples it plays.
pub(crate) fn ogg_samples(ogg: Vec<u8>) -> Result<usize, Box<dyn Error>> {
    let mut reader = OggStreamReader::new(Cursor::new(ogg))?;
    let mut samples = 0;
    while let Some(packet) = reader.read_dec_packet_itl()? {
        samples += packet.len();
    }
    Ok(samples)
}

/// Returns the samples of a canonical 44-byte-header mono PCM WAV.
pub(crate) fn wav_samples(wav: &[u8]) -> Vec<i16> {
    wav.get(44..)
        .unwrap_or_default()
        .as_chunks::<2>()
        .0
        .iter()
        .copied()
        .map(i16::from_le_bytes)
        .collect()
}

/// Returns every file below `folder` whose name ends in `.part`.
pub(crate) fn partial_files(folder: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    let mut pending = vec![folder.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(next)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "part")
            {
                found.push(path);
            }
        }
    }
    Ok(found)
}
