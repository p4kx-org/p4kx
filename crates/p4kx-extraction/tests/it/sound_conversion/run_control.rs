// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::sync::Mutex;

use p4kx_extraction::{AudioConversion, CancelToken, ConflictPolicy, ExtractOptions, Progress};

use p4kx_testkit::{TestBank, TestEntry};

use super::fixture::{BANK, Fixture, PCM, TestResult, VORBIS, XMA, bank, partial_files, pcm};

const OGG: &str = "Data/Sounds/wwise/Media/500.ogg";
const EMBEDDED: &str = "Data/Sounds/wwise/Ship/20.wav";

#[test]
fn skip_keeps_existing_converted_files() -> TestResult {
    let fixture = Fixture::sounds()?;
    fixture.run(AudioConversion::Ogg, ConflictPolicy::Skip);
    fs::write(fixture.target.join(OGG), b"old")?;

    let again = fixture.run(AudioConversion::Ogg, ConflictPolicy::Skip);

    assert_eq!(fs::read(fixture.target.join(OGG))?, b"old", "kept");
    assert_eq!(
        again.skipped(),
        [VORBIS, PCM, XMA, BANK],
        "every sound kept"
    );
    Ok(())
}

#[test]
fn skip_keeps_an_existing_output_without_reading_the_sound() -> TestResult {
    let fixture = Fixture::new(vec![
        TestEntry::zstd(PCM, pcm()).with_payload(vec![0xFF; 16]),
    ])?;
    let output = fixture.target.join("Data/Sounds/wwise/Media/600.wav");

    let read = fixture.run(AudioConversion::Wav, ConflictPolicy::Skip);
    fs::create_dir_all(output.parent().ok_or("folder")?)?;
    fs::write(&output, b"old")?;
    let kept = fixture.run(AudioConversion::Wav, ConflictPolicy::Skip);

    assert_eq!(
        read.failures().len(),
        1,
        "an unreadable sound fails when read"
    );
    assert!(kept.failures().is_empty(), "{:?}", kept.failures());
    assert_eq!(kept.skipped(), [PCM], "kept");
    assert_eq!(fs::read(&output)?, b"old", "untouched");
    Ok(())
}

#[test]
fn skip_writes_a_sound_whose_own_output_is_missing_beside_a_file_of_another_format() -> TestResult {
    let fixture = Fixture::sounds()?;
    let stale = fixture.target.join("Data/Sounds/wwise/Media/600.ogg");
    fs::create_dir_all(stale.parent().ok_or("folder")?)?;
    fs::write(&stale, b"stale")?;

    let report = fixture.run(AudioConversion::Ogg, ConflictPolicy::Skip);

    assert!(
        fixture.exists("Data\\Sounds\\wwise\\Media\\600.wav"),
        "the WAV is written"
    );
    assert!(!report.skipped().contains(&PCM.to_owned()), "not kept");
    Ok(())
}

#[test]
fn if_changed_keeps_unchanged_files_and_replaces_changed_ones() -> TestResult {
    let fixture = Fixture::sounds()?;
    fixture.run(AudioConversion::Ogg, ConflictPolicy::Skip);
    let first = fs::read(fixture.target.join(OGG))?;

    let unchanged = fixture.run(AudioConversion::Ogg, ConflictPolicy::IfChanged);
    fs::write(fixture.target.join(EMBEDDED), b"stale")?;
    let changed = fixture.run(AudioConversion::Ogg, ConflictPolicy::IfChanged);

    assert_eq!(
        unchanged.skipped(),
        [VORBIS, PCM, XMA, BANK],
        "nothing changed"
    );
    assert_eq!(unchanged.converted(), Some(0), "nothing written");
    assert_eq!(
        changed.skipped(),
        [VORBIS, PCM, XMA],
        "only the soundbank is written"
    );
    assert_ne!(
        fs::read(fixture.target.join(EMBEDDED))?,
        b"stale",
        "replaced"
    );
    assert_eq!(fs::read(fixture.target.join(OGG))?, first, "unchanged");
    Ok(())
}

#[test]
fn overwrite_replaces_converted_files() -> TestResult {
    let fixture = Fixture::sounds()?;
    fixture.run(AudioConversion::Ogg, ConflictPolicy::Skip);
    fs::write(fixture.target.join(OGG), b"old")?;
    fs::write(fixture.target.join(EMBEDDED), b"old")?;

    let again = fixture.run(AudioConversion::Ogg, ConflictPolicy::Overwrite);

    assert_ne!(fs::read(fixture.target.join(OGG))?, b"old", "replaced");
    assert_ne!(fs::read(fixture.target.join(EMBEDDED))?, b"old", "replaced");
    assert!(again.skipped().is_empty(), "nothing kept");
    assert_eq!(again.converted(), Some(3), "converted again");
    Ok(())
}

#[test]
fn a_cancelled_run_writes_no_sound_and_leaves_no_partial_file() -> TestResult {
    let fixture = Fixture::sounds()?;
    let cancel = CancelToken::new();
    cancel.cancel();

    let report = fixture.run_with(&ExtractOptions {
        convert: true,
        cancel: Some(&cancel),
        ..ExtractOptions::default()
    });

    assert_eq!(report.cancelled(), [VORBIS, PCM, XMA, BANK], "every sound");
    assert_eq!(report.extracted(), 0, "nothing written");
    assert!(
        partial_files(&fixture.target)?.is_empty(),
        "no partial file"
    );
    Ok(())
}

#[test]
fn cancelling_during_the_run_stops_the_rest_without_partial_files() -> TestResult {
    let fixture = Fixture::sounds()?;
    let cancel = CancelToken::new();
    let on_progress = |progress: Progress| {
        if progress.bytes_done > 0 {
            cancel.cancel();
        }
    };

    let report = fixture.run_with(&ExtractOptions {
        convert: true,
        cancel: Some(&cancel),
        progress: Some(&on_progress),
        ..ExtractOptions::default()
    });

    assert!(!report.cancelled().is_empty(), "the run stopped");
    let cancelled_entries = report
        .cancelled()
        .iter()
        .filter(|path| !path.contains(" WEM "))
        .count();
    assert_eq!(
        report.extracted() + cancelled_entries + report.failures().len(),
        4,
        "every Entry ended once"
    );
    assert!(
        partial_files(&fixture.target)?.is_empty(),
        "no partial file"
    );
    Ok(())
}

#[test]
fn a_soundbank_cancelled_between_sounds_reports_what_it_wrote_and_what_it_left() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::zstd(BANK, bank())])?;
    let first = fixture.target.join("Data/Sounds/wwise/Ship/10.ogg");
    let cancel = CancelToken::new();
    let on_progress = |_: Progress| {
        if first.exists() {
            cancel.cancel();
        }
    };

    let report = fixture.run_with(&ExtractOptions {
        convert: true,
        cancel: Some(&cancel),
        progress: Some(&on_progress),
        ..ExtractOptions::default()
    });

    assert_eq!(report.extracted(), 1, "the soundbank itself is written");
    assert!(fixture.exists(BANK), "the soundbank is on disk");
    assert_eq!(
        report.cancelled(),
        [20, 30, 40].map(|id| format!("{BANK} WEM {id}")),
        "the sounds not written"
    );
    assert!(report.failures().is_empty(), "nothing failed");
    assert!(!report.is_complete(), "the run is not complete");
    assert_eq!(
        fixture.names("Data/Sounds/wwise/Ship")?,
        ["10.ogg", "10.ogg.p4kx-origin.jsonl"],
        "only the sound written before the cancel"
    );
    assert!(
        partial_files(&fixture.target)?.is_empty(),
        "no partial file"
    );
    Ok(())
}

#[test]
fn a_soundbank_without_sounds_creates_no_folder() -> TestResult {
    let fixture = Fixture::new(vec![TestEntry::zstd(BANK, TestBank::new(1).bytes())])?;

    let report = fixture.run(AudioConversion::Ogg, ConflictPolicy::Skip);

    assert_eq!(report.extracted(), 1, "the soundbank is written");
    assert!(fixture.exists(BANK), "the soundbank is on disk");
    assert!(
        !fixture.exists("Data\\Sounds\\wwise\\Ship"),
        "no folder for sounds that are not there"
    );
    Ok(())
}

#[test]
fn progress_counts_every_sound_once_and_reaches_the_total() -> TestResult {
    let fixture = Fixture::sounds()?;
    let seen = Mutex::new(Vec::new());
    let on_progress = |progress: Progress| {
        if let Ok(mut seen) = seen.lock() {
            seen.push(progress);
        }
    };

    fixture.run_with(&ExtractOptions {
        convert: true,
        progress: Some(&on_progress),
        ..ExtractOptions::default()
    });

    let seen = seen.into_inner().map_err(|_| "poisoned")?;
    let last = seen.last().copied().ok_or("progress reported")?;
    assert_eq!(last.entries_done, 4, "every Entry ended");
    assert_eq!(last.bytes_done, last.bytes_total, "every byte counted once");
    assert!(
        seen.windows(2)
            .all(|pair| pair[0].bytes_done <= pair[1].bytes_done),
        "never backwards"
    );
    Ok(())
}
