// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{AudioConversion, ConflictPolicy, EntryFailure, ExtractOptions};
use p4kx_testkit::{TestBank, TestEntry, TestOpus, TestOpusFraming, test_signal};

use super::fixture::{
    BANK, Fixture, PCM, PCM_FRAMES, TestResult, VORBIS, XMA, bank, ogg_samples, pcm, prefetch,
    reason, vorbis, wav_samples, xma,
};

const MEDIA: &str = "Data\\Sounds\\wwise\\Media";
const BANK_FOLDER: &str = "Data\\Sounds\\wwise\\Ship";

#[test]
fn ogg_rebuilds_vorbis_decodes_other_codecs_to_wav_and_unpacks_the_soundbank() -> TestResult {
    let fixture = Fixture::sounds()?;

    let report = fixture.run(AudioConversion::Ogg, ConflictPolicy::Skip);

    assert_eq!(
        fixture.names(MEDIA)?,
        ["500.ogg", "600.wav", "600.wav.p4kx-origin.jsonl", "700.wem"],
        "loose sounds"
    );
    assert!(
        ogg_samples(fixture.read("Data\\Sounds\\wwise\\Media\\500.ogg")?)? > 0,
        "playable Ogg"
    );
    assert_eq!(
        wav_samples(&fixture.read("Data\\Sounds\\wwise\\Media\\600.wav")?),
        test_signal(PCM_FRAMES, 1),
        "the known signal"
    );
    assert_eq!(
        fixture.read("Data\\Sounds\\wwise\\Media\\700.wem")?,
        xma(),
        "written unchanged"
    );
    assert_eq!(
        fixture.read(BANK)?,
        bank(),
        "the soundbank itself unchanged"
    );
    assert_eq!(
        fixture.names(BANK_FOLDER)?,
        ["10.ogg", "20.wav", "30.wem", "40.wem"],
        "one file per embedded WEM, named by ID"
    );
    assert!(
        ogg_samples(fixture.read("Data\\Sounds\\wwise\\Ship\\10.ogg")?)? > 0,
        "playable Ogg"
    );
    assert_eq!(
        fixture.read("Data\\Sounds\\wwise\\Ship\\40.wem")?,
        prefetch(),
        "the prefetched start"
    );
    assert_eq!(report.extracted(), 4, "every Entry");
    assert_eq!(
        report.converted(),
        Some(3),
        "both decodable sounds and the soundbank"
    );
    Ok(())
}

#[test]
fn a_sound_whose_codec_cannot_be_decoded_is_reported_with_its_codec() -> TestResult {
    let fixture = Fixture::sounds()?;

    let report = fixture.run(AudioConversion::Ogg, ConflictPolicy::Skip);

    let unconverted: Vec<(&str, String)> = report
        .unconverted()
        .iter()
        .map(|failure| (failure.path(), reason(failure)))
        .collect();
    assert_eq!(unconverted.len(), 2, "{unconverted:?}");
    assert_eq!(unconverted[0].0, XMA, "the loose WEM");
    assert!(unconverted[0].1.contains("XMA2"), "{}", unconverted[0].1);
    assert_eq!(unconverted[1].0, BANK, "the soundbank");
    assert!(
        unconverted[1].1.contains("Ship.bnk WEM 30") && unconverted[1].1.contains("XMA2"),
        "{}",
        unconverted[1].1
    );
    assert!(report.failures().is_empty(), "no failure");
    assert!(!report.is_complete(), "not every sound was converted");
    Ok(())
}

#[test]
fn wav_decodes_every_sound() -> TestResult {
    let fixture = Fixture::sounds()?;

    let report = fixture.run(AudioConversion::Wav, ConflictPolicy::Skip);

    assert_eq!(
        fixture.names(MEDIA)?,
        ["500.wav", "600.wav", "600.wav.p4kx-origin.jsonl", "700.wem"],
        "loose sounds"
    );
    assert_eq!(
        fixture.names(BANK_FOLDER)?,
        ["10.wav", "20.wav", "30.wem", "40.wem"],
        "embedded WEMs"
    );
    assert_eq!(
        wav_samples(&fixture.read("Data\\Sounds\\wwise\\Ship\\20.wav")?),
        test_signal(PCM_FRAMES, 1),
        "the known signal"
    );
    assert_eq!(report.converted(), Some(3), "converted");
    Ok(())
}

#[test]
fn raw_keeps_the_sounds_and_unpacks_the_soundbank_unchanged() -> TestResult {
    let fixture = Fixture::sounds()?;

    let report = fixture.run(AudioConversion::Raw, ConflictPolicy::Skip);

    assert_eq!(
        fixture.names(MEDIA)?,
        ["500.wem", "600.wem", "700.wem"],
        "unchanged"
    );
    assert_eq!(fixture.read(VORBIS)?, vorbis(), "the Entry");
    assert_eq!(
        fixture.names(BANK_FOLDER)?,
        ["10.wem", "20.wem", "30.wem", "40.wem"],
        "embedded WEMs as they are"
    );
    assert_eq!(
        fixture.read("Data\\Sounds\\wwise\\Ship\\20.wem")?,
        pcm(),
        "the bytes"
    );
    assert!(
        report.is_complete(),
        "nothing to report: {:?}",
        report.unconverted()
    );
    assert_eq!(report.converted(), Some(0), "nothing was converted");
    assert_eq!(report.extracted(), 4, "the soundbank was still written");
    Ok(())
}

#[test]
fn a_soundbank_whose_wems_all_fall_back_to_raw_is_not_counted_converted() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::zstd(
        BANK,
        TestBank::new(1).wem(30, xma()).bytes(),
    )])?;

    let report = fixture.run(AudioConversion::Ogg, ConflictPolicy::Skip);

    assert_eq!(fixture.names(BANK_FOLDER)?, ["30.wem"], "written unchanged");
    assert_eq!(report.converted(), Some(0), "not converted");
    assert_eq!(report.unconverted().len(), 1, "listed as not converted");
    Ok(())
}

#[test]
fn without_conversion_sounds_and_soundbanks_are_written_unchanged() -> TestResult {
    let fixture = Fixture::sounds()?;

    let report = fixture.run_with(&ExtractOptions::default());

    assert_eq!(
        fixture.names(MEDIA)?,
        ["500.wem", "600.wem", "700.wem"],
        "unchanged"
    );
    assert!(!fixture.exists(BANK_FOLDER), "no soundbank folder");
    assert_eq!(fixture.read(PCM)?, pcm(), "the Entry");
    assert!(report.is_complete(), "complete");
    Ok(())
}

#[test]
fn a_soundbank_that_cannot_be_read_is_reported_and_written_unchanged() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::stored(BANK, b"BKHD broken".to_vec())])?;

    let report = fixture.run(AudioConversion::Ogg, ConflictPolicy::Skip);

    assert_eq!(fixture.read(BANK)?, b"BKHD broken", "written unchanged");
    assert!(!fixture.exists(BANK_FOLDER), "nothing unpacked");
    assert_eq!(report.unconverted().len(), 1, "reported");
    assert_eq!(report.converted(), Some(0), "not converted");
    assert_eq!(report.extracted(), 1, "but written");
    Ok(())
}

#[test]
fn a_file_inside_the_soundbank_folder_is_written_after_the_soundbank() -> TestResult {
    const INSIDE: &str = "Data\\Sounds\\wwise\\Ship\\10.ogg";
    let fixture = Fixture::new(vec![
        TestEntry::stored(BANK, bank()),
        TestEntry::stored(INSIDE, b"loose".to_vec()),
    ])?;

    let report = fixture.run(AudioConversion::Ogg, ConflictPolicy::Skip);

    assert_eq!(
        report.skipped(),
        [INSIDE],
        "the soundbank's file came first"
    );
    assert!(
        ogg_samples(fixture.read(INSIDE)?)? > 0,
        "the unpacked Ogg stays"
    );
    Ok(())
}

#[test]
fn many_sounds_are_converted_in_parallel_with_one_report_in_entry_order() -> TestResult {
    let paths: Vec<String> = (0..48)
        .map(|number| format!("Data\\Sounds\\wwise\\Media\\{number:03}.wem"))
        .collect();
    let fixture = Fixture::new(
        paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let bytes = if index % 3 == 0 { xma() } else { pcm() };
                TestEntry::zstd(path, bytes)
            })
            .collect(),
    )?;

    let report = fixture.run(AudioConversion::Ogg, ConflictPolicy::Skip);

    assert_eq!(report.extracted(), 48, "every sound");
    assert_eq!(report.converted(), Some(32), "every PCM sound");
    let unconverted: Vec<&str> = report
        .unconverted()
        .iter()
        .map(EntryFailure::path)
        .collect();
    let expected: Vec<&str> = paths.iter().step_by(3).map(String::as_str).collect();
    assert_eq!(unconverted, expected, "in Entry order");
    Ok(())
}

#[test]
fn opus_is_repackaged_as_ogg_opus_and_decoded_to_wav_on_request() -> TestResult {
    const OPUS: &str = "Data\\Sounds\\wwise\\Media\\800.wem";
    let opus = TestOpus::new(TestOpusFraming::Wwise, 2, 4_800)
        .wem()?
        .bytes();
    let soundbank = TestBank::new(1).wem(50, opus.clone()).bytes();
    let entries = || {
        vec![
            TestEntry::zstd(OPUS, opus.clone()),
            TestEntry::stored(BANK, soundbank.clone()),
        ]
    };
    let ogg = Fixture::new(entries())?;
    let wav = Fixture::new(entries())?;

    let ogg_report = ogg.run(AudioConversion::Ogg, ConflictPolicy::Skip);
    let wav_report = wav.run(AudioConversion::Wav, ConflictPolicy::Skip);

    for path in [
        "Data\\Sounds\\wwise\\Media\\800.ogg",
        "Data\\Sounds\\wwise\\Ship\\50.ogg",
    ] {
        let written = ogg.read(path)?;
        assert!(written.starts_with(b"OggS"), "{path} is Ogg");
        assert!(
            written.windows(8).any(|window| window == b"OpusHead"),
            "{path} is Ogg Opus"
        );
    }
    assert_eq!(ogg_report.converted(), Some(2), "both converted");
    assert!(ogg_report.is_complete(), "{:?}", ogg_report.unconverted());
    for path in [
        "Data\\Sounds\\wwise\\Media\\800.wav",
        "Data\\Sounds\\wwise\\Ship\\50.wav",
    ] {
        assert!(wav.read(path)?.starts_with(b"RIFF"), "{path} is WAV");
    }
    assert!(wav_report.is_complete(), "{:?}", wav_report.unconverted());
    Ok(())
}
