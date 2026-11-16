// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Mutex;

use p4kx_extraction::{CancelToken, ConflictPolicy, ExtractOptions, Progress, extract};

use p4kx_testkit::{SplitDds, TestTextureFormat};

use super::fixture::{Fixture, HEADER, files, merged_layout, rock, run, texture_entries};

type TestResult = Result<(), Box<dyn Error>>;

const DDS: &str = "Data/Textures/rock_diff.dds";
const PNG: &str = "Data/Textures/rock_diff.png";
const ENTRIES: [&str; 4] = [
    "Data\\Textures\\rock_diff.dds",
    "Data\\Textures\\rock_diff.dds.1",
    "Data\\Textures\\rock_diff.dds.2",
    "Data\\Textures\\rock_diff.dds.3",
];

fn converted_once() -> Result<Fixture, Box<dyn Error>> {
    let fixture = Fixture::new(texture_entries(&rock()))?;
    let report = run(&fixture, ConflictPolicy::Skip);
    assert!(report.is_complete(), "{report:?}");
    Ok(fixture)
}

#[test]
fn skip_keeps_existing_outputs_and_reports_the_texture_skipped() -> TestResult {
    let fixture = converted_once()?;
    fixture.write(PNG, b"old")?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert_eq!(report.skipped(), ENTRIES, "every Entry of the texture");
    assert_eq!(report.converted(), Some(0), "nothing written");
    assert_eq!(fixture.read(PNG)?, b"old", "kept");
    Ok(())
}

#[test]
fn skip_writes_an_output_that_is_missing() -> TestResult {
    let fixture = converted_once()?;
    std::fs::remove_file(fixture.target.join(PNG))?;
    fixture.write(DDS, b"old")?;

    let report = run(&fixture, ConflictPolicy::Skip);

    assert_eq!(report.converted(), Some(4), "the PNG was written");
    assert_eq!(fixture.read(DDS)?, b"old", "the merged DDS is kept");
    assert!(fixture.target.join(PNG).exists(), "the PNG is back");
    Ok(())
}

#[test]
fn overwrite_replaces_existing_outputs() -> TestResult {
    let fixture = converted_once()?;
    let png = fixture.read(PNG)?;
    fixture.write(DDS, b"old")?;
    fixture.write(PNG, b"old")?;

    let report = run(&fixture, ConflictPolicy::Overwrite);

    assert_eq!(report.converted(), Some(4), "converted again");
    assert_eq!(fixture.read(DDS)?, merged_layout(&rock()), "merged DDS");
    assert_eq!(fixture.read(PNG)?, png, "PNG");
    Ok(())
}

#[test]
fn if_changed_keeps_identical_outputs_and_replaces_changed_ones() -> TestResult {
    let fixture = converted_once()?;

    let unchanged = run(&fixture, ConflictPolicy::IfChanged);
    let mut edited = fixture.read(DDS)?;
    if let Some(last) = edited.last_mut() {
        *last ^= 0xFF;
    }
    fixture.write(DDS, &edited)?;
    let changed = run(&fixture, ConflictPolicy::IfChanged);

    assert_eq!(unchanged.skipped(), ENTRIES, "identical outputs are kept");
    assert_eq!(changed.converted(), Some(4), "the edited DDS is rewritten");
    assert_eq!(fixture.read(DDS)?, merged_layout(&rock()), "restored");
    assert_eq!(
        files(&fixture.target)?,
        [DDS, "Data/Textures/rock_diff.dds.p4kx-origin.jsonl", PNG],
        "no temporary files left"
    );
    Ok(())
}

#[test]
fn progress_counts_every_entry_and_byte_of_a_converted_texture_once() -> TestResult {
    let fixture = Fixture::new(texture_entries(&rock()))?;
    let last = Mutex::new(Progress::default());
    let on_progress = |now: Progress| {
        if let Ok(mut last) = last.lock() {
            *last = now;
        }
    };
    let entries: Vec<_> = fixture.archive.entries().iter().collect();
    let options = ExtractOptions {
        convert: true,
        progress: Some(&on_progress),
        ..ExtractOptions::default()
    };

    let report = extract(&fixture.archive, &entries, &fixture.target, &options);

    assert!(report.is_complete(), "{report:?}");
    let last = *last.lock().map_err(|_| "poisoned")?;
    assert_eq!(last.entries_done, 4, "every Entry ended");
    assert_eq!(last.entries_total, 4, "of four");
    assert_eq!(last.bytes_done, last.bytes_total, "every byte counted once");
    Ok(())
}

#[test]
fn a_cancelled_texture_conversion_writes_nothing() -> TestResult {
    let fixture = Fixture::new(texture_entries(&rock()))?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let entries: Vec<_> = fixture.archive.entries().iter().collect();
    let options = ExtractOptions {
        convert: true,
        cancel: Some(&cancel),
        ..ExtractOptions::default()
    };

    let report = extract(&fixture.archive, &entries, &fixture.target, &options);

    assert_eq!(report.cancelled(), ENTRIES, "every Entry is cancelled");
    assert!(report.unconverted().is_empty(), "no texture failed");
    assert!(files(&fixture.target)?.is_empty(), "nothing written");
    Ok(())
}

#[test]
fn a_cancel_in_the_middle_of_a_merge_leaves_no_file_and_counts_progress_once() -> TestResult {
    let big = SplitDds::new(TestTextureFormat::LegacyBgra8, 512, 512).split(2);
    let fixture = Fixture::new(big.entries(HEADER))?;
    let cancel = CancelToken::new();
    let last = Mutex::new(Progress::default());
    let on_progress = |now: Progress| {
        if now.entries_done == 0 && now.bytes_done > 0 {
            cancel.cancel();
        }
        if let Ok(mut last) = last.lock() {
            *last = now;
        }
    };
    let entries: Vec<_> = fixture.archive.entries().iter().collect();
    let options = ExtractOptions {
        convert: true,
        cancel: Some(&cancel),
        progress: Some(&on_progress),
        ..ExtractOptions::default()
    };

    let report = extract(&fixture.archive, &entries, &fixture.target, &options);

    assert!(cancel.is_cancelled(), "progress arrived while merging");
    assert_eq!(
        report.cancelled(),
        &ENTRIES[..3],
        "every Entry is cancelled"
    );
    assert!(
        files(&fixture.target)?.is_empty(),
        "no partial or temporary file"
    );
    let last = *last.lock().map_err(|_| "poisoned")?;
    assert_eq!(last.entries_done, 3, "every Entry ended");
    assert_eq!(last.bytes_done, last.bytes_total, "every byte counted once");
    Ok(())
}
