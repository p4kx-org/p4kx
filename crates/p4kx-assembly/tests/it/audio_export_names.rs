// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The names of exported sounds follow the portable file-name rule of ADR-0248.

use std::error::Error;

use p4kx_assembly::audio::{
    ArchiveAudio, ExportFailureKind, ExportOptions, ExportReport, ExportReportCapture,
    ExportReportOrigins, ExportWatch, SoundFormat,
};
use p4kx_extraction::{Archive, ConflictPolicy, check_part};
use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestCodec, TestEntry, TestHircObject, TestWem, test_signal,
    wwise_short_id,
};

type TestResult = Result<(), Box<dyn Error>>;

const PLAY: u16 = 0x0403;
const OPTIONS: ExportOptions = ExportOptions {
    format: SoundFormat::Wav,
    policy: ConflictPolicy::Overwrite,
};

/// Exports the sound of each trigger of `names`, which all post one event, into the folder
/// `folder` of a temporary directory, and checks that every file name passes `check_part`.
fn export(names: &[&str], folder: &str) -> Result<ExportReport, Box<dyn Error>> {
    export_with_origins(names, folder).map(|(report, _)| report)
}

fn export_with_origins(
    names: &[&str],
    folder: &str,
) -> Result<(ExportReport, ExportReportOrigins), Box<dyn Error>> {
    let wem = TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(480, 1)).bytes();
    let size = u32::try_from(wem.len())?;
    let bank = TestBank::new(wwise_short_id("Bank"))
        .wem(7001, wem)
        .object(TestHircObject::event(wwise_short_id("Play_E"), &[1]))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(TestHircObject::sound(10, 7001, 0, size));
    let atl = names.iter().fold(TestAtl::new("SC"), |atl, name| {
        atl.trigger(*name, &["Play_E"])
    });
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\SC.xml",
            atl.to_cryxml(),
        ))
        .entry(TestEntry::stored(
            "Data\\Sounds\\wwise\\Bank.bnk",
            bank.bytes(),
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let audio = ArchiveAudio::new(&archive);
    let resolved: Vec<_> = names
        .iter()
        .map(|name| audio.resolve_trigger(name).ok_or("resolves"))
        .collect::<Result<_, _>>()?;
    let triggers: Vec<_> = resolved.iter().collect();
    let out = tempfile::tempdir()?;
    let (report, origins) = audio.export_sounds_watched_with_report_origins(
        &triggers,
        &out.path().join(folder),
        OPTIONS,
        &ExportWatch::default(),
        ExportReportCapture::FileSizes,
    )?;
    for sound in &report.sounds {
        let name = sound.path.file_name().and_then(|name| name.to_str());
        assert_eq!(check_part(name.ok_or("a name")?), Ok(()), "{name:?}");
    }
    Ok((report, origins))
}

fn names(report: &ExportReport) -> Vec<&str> {
    report
        .sounds
        .iter()
        .filter_map(|sound| sound.path.file_name()?.to_str())
        .collect()
}

#[test]
fn a_forbidden_character_or_device_name_is_escaped() -> TestResult {
    let report = export(&["Play|X:Y", "CON", "a."], "out")?;
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(
        names(&report),
        ["Play%7CX%3AY.wav", "%43ON.wav", "a%2E.wav"]
    );
    Ok(())
}

#[test]
fn names_that_differ_only_in_case_stay_distinct() -> TestResult {
    let report = export(&["Ärger", "ärger"], "out")?;
    assert_eq!(names(&report), ["Ärger.wav", "ärger_2.wav"]);
    Ok(())
}

#[test]
fn a_name_too_long_for_a_file_name_becomes_sound() -> TestResult {
    let long = "T".repeat(300);
    let report = export(&[&long, "Play_X"], "out")?;
    assert_eq!(names(&report), ["sound.wav", "Play_X.wav"]);
    Ok(())
}

#[test]
fn a_sound_whose_path_exceeds_the_limit_is_reported_and_the_others_are_written() -> TestResult {
    let long = "T".repeat(220);
    let (report, origins) = export_with_origins(&[&long, "Play_X"], &"F".repeat(30))?;
    assert_eq!(names(&report), ["Play_X.wav"]);
    let [failure] = report.failures.as_slice() else {
        return Err(format!("one sound fails: {:?}", report.failures).into());
    };
    let [origin] = origins.failures() else {
        return Err("one bound job failure".into());
    };
    assert_eq!(
        (origin.trigger_ordinal, origin.variant_ordinal, origin.kind),
        (0, 0, ExportFailureKind::Job),
        "the failed write keeps its original job position"
    );
    assert_eq!(
        failure.trigger, long,
        "the existing failure carries its trigger name"
    );
    assert_eq!(failure.wem_id, Some(7001));
    assert!(
        failure.reason.contains("UTF-16 units"),
        "{}",
        failure.reason
    );
    Ok(())
}
