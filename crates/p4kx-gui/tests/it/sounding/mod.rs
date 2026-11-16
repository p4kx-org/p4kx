// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Helpers of the audio preview tests: a synthetic Archive with WEMs opened in a headless
//! window that plays through a fake audio output.

use crate::waiting::wait_for_model;
use std::error::Error;
use std::sync::{Arc, Mutex, PoisonError};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{
    ArchiveKey, AudioOutput, AudioPreview, Browser, Language, Model, Player, Preview, SoundSource,
    StreamGuard, ui,
};
use p4kx_testkit::{
    ArchiveBuilder, TestCodec, TestEntry, TestOpus, TestOpusFraming, TestVorbis, TestWem,
    test_signal,
};
use tempfile::TempDir;

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

/// The frames of the PCM test sound, one second at 48 kHz.
const TONE_FRAMES: u32 = 48_000;

/// An audio output that keeps the sources it was started with, failing them with `failure`.
#[derive(Debug, Clone, Default)]
pub(crate) struct FakeOutput {
    pub(crate) sources: Arc<Mutex<Vec<SoundSource>>>,
    pub(crate) failure: Option<&'static str>,
}

impl FakeOutput {
    pub(crate) fn started(&self) -> usize {
        self.sources
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Pulls `count` samples from the latest source; `None` once it ended.
    pub(crate) fn pull(&self, count: usize) -> Option<Vec<f32>> {
        let mut sources = self.sources.lock().unwrap_or_else(PoisonError::into_inner);
        let source = sources.last_mut()?;
        let mut samples = vec![0.0; count];
        let written = source.fill(&mut samples);
        drop(sources);
        (written == count).then_some(samples)
    }
}

impl AudioOutput for FakeOutput {
    fn start(&self, source: SoundSource) -> StreamGuard {
        if let Some(reason) = self.failure {
            source.reporter().report(reason);
        }
        self.sources
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(source);
        StreamGuard::new(())
    }
}

fn tone() -> Vec<i16> {
    test_signal(TONE_FRAMES, 2)
}

/// A Wwise Vorbis tone of two channels.
fn vorbis() -> TestVorbis {
    TestVorbis::new(2, 48_000, &[false; 400]).tone(10)
}

/// A Wwise Opus sound of two channels, 0.2 seconds of [`test_signal`].
pub(crate) const fn opus() -> TestOpus {
    TestOpus::new(TestOpusFraming::Wwise, 2, 9_600)
}

fn sounds() -> Result<Vec<TestEntry>, Box<dyn Error>> {
    let pcm = TestWem::pcm(TestCodec::WwisePcm, 2, 48_000, &tone()).bytes();
    let ima = TestWem::ima_adpcm(1, 48_000, &test_signal(640, 1)).bytes();
    let ptadpcm = TestWem::ptadpcm(1, 48_000, 36, &test_signal(640, 1)).bytes();
    let aac = TestWem::encoded(TestCodec::Tag(0xAAC0), 1, 48_000, 960).bytes();
    let endless = TestWem::encoded(TestCodec::Vorbis, 2, 48_000, u32::MAX).bytes();
    let looping = TestWem::pcm(TestCodec::WwisePcm, 2, 48_000, &tone())
        .smpl_loop(0, 12_000, 35_999)
        .bytes();
    Ok(vec![
        TestEntry::stored("Data\\Sounds\\tone.wem", pcm),
        TestEntry::stored("Data\\Sounds\\looping.wem", looping),
        TestEntry::zstd("Data\\Sounds\\ima.WEM", ima),
        TestEntry::stored("Data\\Sounds\\platinum.wem", ptadpcm),
        TestEntry::stored("Data\\Sounds\\engine.wem", vorbis().wem().bytes()),
        TestEntry::stored("Data\\Sounds\\voice.wem", opus().wem()?.bytes()),
        TestEntry::stored("Data\\Sounds\\console.wem", aac),
        TestEntry::stored("Data\\Sounds\\endless.wem", endless),
        TestEntry::stored(
            "Data\\Sounds\\broken.wem",
            b"RIFF\x04\x00\x00\x00WAVE".to_vec(),
        ),
        TestEntry::stored("Data\\readme.txt", "plain text"),
    ])
}

pub(crate) fn opened(
    language: Language,
    output: &FakeOutput,
) -> Result<(TempDir, Harness<'static, Model>), Box<dyn Error>> {
    opened_with(language, output, Vec::new())
}

/// Opens the synthetic Archive of the sounds with the Entries `extra` added.
pub(crate) fn opened_with(
    language: Language,
    output: &FakeOutput,
    extra: Vec<TestEntry>,
) -> Result<(TempDir, Harness<'static, Model>), Box<dyn Error>> {
    opened_with_key(language, output, extra, None)
}

pub(crate) fn opened_with_key(
    language: Language,
    output: &FakeOutput,
    extra: Vec<TestEntry>,
    key: Option<ArchiveKey>,
) -> Result<(TempDir, Harness<'static, Model>), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    sounds()?
        .into_iter()
        .chain(extra)
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    let mut model = Model::new(language);
    model.set_archive_key(key);
    model.set_audio_output(output.clone());
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1600.0, 900.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    harness.state_mut().open(path, || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    Ok((folder, harness))
}

pub(crate) fn wait_until(
    harness: &mut Harness<'_, Model>,
    done: impl Fn(&Model) -> bool,
) -> TestResult {
    wait_for_model(harness, "the window to settle", done)?;
    harness.step();
    Ok(())
}

pub(crate) fn audio(model: &Model) -> Option<&AudioPreview> {
    model
        .browser()
        .and_then(Browser::preview)
        .and_then(Preview::audio)
}

pub(crate) fn player(model: &Model) -> Option<&Player> {
    audio(model).and_then(AudioPreview::player)
}

pub(crate) fn select(harness: &mut Harness<'_, Model>, name: &str) -> TestResult {
    harness.get_by_label(name).click();
    wait_until(harness, |model| {
        audio(model).is_some_and(|audio| !audio.is_busy())
    })
}
