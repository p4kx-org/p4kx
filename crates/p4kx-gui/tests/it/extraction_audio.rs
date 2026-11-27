// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The sound formats of the Extraction setup, driven headless through `egui_kittest`.

use crate::common::settle;
use crate::extraction_outputs::assert_receipted_outputs;
use crate::waiting::wait_for_model;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{Extraction, ExtractionStatus, Language, Model, ui};
use p4kx_settings::ExtractionAudio;
use p4kx_testkit::{
    ArchiveBuilder, TestBank, TestCodec, TestEntry, TestVorbis, TestWem, test_signal,
};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

const XMA: &str = "Data\\Sounds\\wwise\\Media\\700.wem";
const OGG: &str = "Ogg";
const WAV: &str = "WAV";
const RAW: &str = "WEM, unchanged";

fn vorbis() -> Vec<u8> {
    TestVorbis::new(1, 24_000, &[false, true, true, false, false])
        .wem()
        .bytes()
}

fn pcm() -> Vec<u8> {
    TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(480, 1)).bytes()
}

fn xma() -> Vec<u8> {
    TestWem::encoded(TestCodec::Tag(0x0166), 2, 48_000, 1_000).bytes()
}

fn opened(
    audio: ExtractionAudio,
) -> Result<(TempDir, PathBuf, Harness<'static, Model>), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let bank = TestBank::new(1).wem(10, vorbis()).wem(20, pcm()).bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data\\Sounds\\wwise\\Media\\500.wem",
            vorbis(),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Media\\600.wem",
            pcm(),
        ))
        .entry(TestEntry::zstd(XMA, xma()))
        .entry(TestEntry::zstd("Data\\Sounds\\wwise\\Ship.bnk", bank))
        .write_to(&path)?;
    let out = dir.path().join("out");
    fs::create_dir(&out)?;
    let mut model = Model::new(Language::BritishEnglish);
    model.set_extraction_folder(Some(out.clone()));
    model.set_extraction_conversion(true);
    model.set_extraction_audio(audio);
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1400.0, 1100.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            model,
        );
    harness.state_mut().open(path, || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    Ok((dir, out, harness))
}

fn wait_until(harness: &mut Harness<'_, Model>, done: impl Fn(&Model) -> bool) -> TestResult {
    wait_for_model(harness, "the window to settle", done)?;
    settle(harness);
    Ok(())
}

fn extract_shown(harness: &mut Harness<'_, Model>, clicks: &[&str]) -> TestResult {
    harness.get_by_label("Extract shown…").click();
    settle(harness);
    harness.get_by_label("Existing folder").click();
    settle(harness);
    for label in clicks {
        harness.get_by_label(label).click();
        settle(harness);
    }
    harness.get_by_label("Extract").click();
    settle(harness);
    harness.get_by_label("Extract whole Archive").click();
    wait_until(harness, |model| {
        model
            .extraction()
            .is_some_and(|extraction| !extraction.is_running())
    })
}

fn media(out: &Path) -> PathBuf {
    out.join("Data/Sounds/wwise/Media")
}

fn bank_folder(out: &Path) -> PathBuf {
    out.join("Data/Sounds/wwise/Ship")
}

#[test]
fn the_default_writes_ogg_and_wav_unpacks_soundbanks_and_lists_the_undecodable_sound() -> TestResult
{
    let (_dir, out, mut harness) = opened(ExtractionAudio::default())?;

    extract_shown(&mut harness, &[])?;

    harness.get_by_label("Extracted 4 (3 converted), skipped 0, failed 0, cancelled 0");
    harness.get_by_label("Not converted (1)");
    assert_summary_receipts(&harness)?;
    harness.get_by_label(&format!(
        "{XMA}: cannot convert the sound: {XMA}: converting XMA2 to WAV is not supported"
    ));
    assert_receipted_outputs(&media(&out), &["500.ogg", "600.wav", "700.wem"])?;
    assert_receipted_outputs(&bank_folder(&out), &["10.ogg", "20.wav"])
}

fn assert_summary_receipts(harness: &Harness<'_, Model>) -> TestResult {
    harness.get_by_label("File origin receipts (6)");
    let Some(ExtractionStatus::Finished(summary)) =
        harness.state().extraction().map(Extraction::status)
    else {
        return Err("expected a finished extraction summary".into());
    };
    let receipts = summary.file_origin_receipts();
    for name in [
        "500.ogg", "600.wav", "700.wem", "Ship.bnk", "10.ogg", "20.wav",
    ] {
        let companion = format!("{name}.p4kx-origin.jsonl");
        assert!(
            receipts.iter().any(|path| path.ends_with(&companion)),
            "the summary names the actual audio companion for {name}"
        );
    }
    Ok(())
}

#[test]
fn choosing_wav_writes_wav_and_is_remembered() -> TestResult {
    let (_dir, out, mut harness) = opened(ExtractionAudio::default())?;

    extract_shown(&mut harness, &[WAV])?;

    assert_receipted_outputs(&media(&out), &["500.wav", "600.wav", "700.wem"])?;
    assert_receipted_outputs(&bank_folder(&out), &["10.wav", "20.wav"])?;
    assert_eq!(
        harness.state().extraction_audio(),
        ExtractionAudio::Wav,
        "remembered for the next setup"
    );
    Ok(())
}

#[test]
fn the_setup_starts_with_the_stored_format() -> TestResult {
    let (_dir, out, mut harness) = opened(ExtractionAudio::Raw)?;

    harness.get_by_label("Extract shown…").click();
    settle(&mut harness);
    assert_eq!(
        harness
            .state()
            .extraction_setup()
            .map(p4kx_gui::ExtractionSetup::audio),
        Some(ExtractionAudio::Raw),
        "preset from the settings"
    );
    harness.get_by_label("Close").click();
    settle(&mut harness);
    extract_shown(&mut harness, &[])?;

    assert_receipted_outputs(&media(&out), &["500.wem", "600.wem", "700.wem"])?;
    assert_receipted_outputs(&bank_folder(&out), &["10.wem", "20.wem"])
}

#[test]
fn the_formats_are_offered_only_with_conversion_on() -> TestResult {
    let (_dir, _out, mut harness) = opened(ExtractionAudio::default())?;

    harness.get_by_label("Extract shown…").click();
    settle(&mut harness);
    for label in [OGG, WAV, RAW] {
        harness.get_by_label(label);
    }
    harness
        .get_by_label("Convert CryXml and DataForge to XML, textures and sounds")
        .click();
    settle(&mut harness);

    assert!(harness.query_by_label(OGG).is_none(), "hidden");
    Ok(())
}
