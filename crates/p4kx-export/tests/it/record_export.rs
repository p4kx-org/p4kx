// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Record export through the public interface of `p4kx-export`.

use crate::common;

use std::error::Error;
use std::fs;
use std::path::Path;

use common::database;
use p4kx_export::{
    CancelToken, ConflictPolicy, NDJSON_FILE, RecordExportError, RecordExportOptions,
    RecordExportSummary, RecordFailure, RecordForm, write_record_export,
};
use p4kx_extraction::DataForge;

type TestResult = Result<(), Box<dyn Error>>;

pub(super) fn export(
    bytes: &[u8],
    form: RecordForm,
    target: &Path,
    options: &RecordExportOptions<'_>,
) -> Result<RecordExportSummary, Box<dyn Error>> {
    let forge = DataForge::open(bytes)?;
    let records: Vec<_> = forge.records().collect();
    Ok(write_record_export(&forge, &records, form, target, options))
}

fn failed_guids(summary: &RecordExportSummary) -> Vec<String> {
    summary
        .failures()
        .iter()
        .map(|failure| failure.guid().to_string())
        .collect()
}

#[test]
fn json_writes_one_file_per_record_under_its_record_path() -> TestResult {
    let bytes = database().build();
    let dir = tempfile::tempdir()?;

    let summary = export(
        &bytes,
        RecordForm::Json,
        dir.path(),
        &RecordExportOptions::default(),
    )?;

    assert_eq!(summary.form(), RecordForm::Json, "the form");
    assert_eq!(summary.written(), 3, "every readable Record");
    assert_eq!(failed_guids(&summary), [BROKEN_TEXT], "the unreadable one");
    assert_eq!(
        summary.error_values(),
        3,
        "two bad enums, one dangling Reference"
    );
    assert_eq!(summary.records_with_errors(), 3, "both items and the ship");
    let listing = tree(dir.path())?;
    let files: Vec<&str> = listing
        .lines()
        .filter(|line| line.starts_with("== "))
        .collect();
    assert_eq!(
        files,
        [
            "== libs/foundry/records/items/boxes/Item.Box.json",
            "== libs/foundry/records/items/boxes/Item.Crate.json",
            "== libs/foundry/records/ships/avenger.json",
        ],
        "ADR-0008 names with .json"
    );
    let avenger: serde_json::Value = serde_json::from_slice(&fs::read(
        dir.path().join("libs/foundry/records/ships/avenger.json"),
    )?)?;
    assert_eq!(avenger["$guid"], AVENGER_TEXT, "the GUID");
    assert_eq!(avenger["maker"]["type"], "Item", "a Reference");
    assert_eq!(avenger["wheels"][1]["size"], 17, "an owned child");
    assert!(avenger["trailer"].is_null(), "a null pointer");
    assert!(
        avenger["parts"][2]["error"].is_string(),
        "an error in an array"
    );
    Ok(())
}

const AVENGER_TEXT: &str = "cbee34fd-6441-4000-b99f-924f7f26e19f";
const BROKEN_TEXT: &str = "00000009-0002-0003-0004-000000000005";

#[test]
fn ndjson_writes_one_line_per_record_in_record_path_order() -> TestResult {
    let bytes = database().build();
    let dir = tempfile::tempdir()?;

    let summary = export(
        &bytes,
        RecordForm::Ndjson,
        dir.path(),
        &RecordExportOptions::default(),
    )?;

    assert_eq!(summary.written(), 3, "three lines");
    assert_eq!(failed_guids(&summary), [BROKEN_TEXT], "the unreadable one");
    assert!(!summary.is_complete(), "a Record failed");
    let text = fs::read_to_string(dir.path().join(NDJSON_FILE))?;
    let names: Vec<String> = text
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .map(|value| format!("{} {}", value["$path"], value["$type"]))
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(
        names,
        [
            "\"libs/foundry/records/items/boxes\" \"Item\"",
            "\"libs/foundry/records/items/boxes\" \"Item\"",
            "\"libs/foundry/records/ships/avenger\" \"Ship\"",
        ],
        "record-path order"
    );
    assert!(text.ends_with("}\n"), "each line ends with LF");
    assert!(
        text.lines()
            .next()
            .is_some_and(|line| line.contains("50e8a9f6")),
        "Item.Box before Item.Crate: {text}"
    );
    assert_eq!(
        fs::read_dir(dir.path())?.count(),
        1,
        "no temporary file is left"
    );
    Ok(())
}

#[test]
fn the_same_records_give_identical_bytes() -> TestResult {
    let bytes = database().build();
    for form in RecordForm::ALL {
        let first = tempfile::tempdir()?;
        let second = tempfile::tempdir()?;

        export(&bytes, form, first.path(), &RecordExportOptions::default())?;
        export(&bytes, form, second.path(), &RecordExportOptions::default())?;

        assert_eq!(
            tree(first.path())?,
            tree(second.path())?,
            "{form}: two runs, identical bytes"
        );
    }
    Ok(())
}

#[test]
fn ndjson_order_does_not_depend_on_the_selection_order() -> TestResult {
    let bytes = database().build();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let reversed: Vec<_> = records
        .iter()
        .rev()
        .chain(records.iter())
        .copied()
        .collect();
    let first = tempfile::tempdir()?;
    let second = tempfile::tempdir()?;

    let options = RecordExportOptions::default();
    let in_order =
        write_record_export(&forge, &records, RecordForm::Ndjson, first.path(), &options);
    let summary = write_record_export(
        &forge,
        &reversed,
        RecordForm::Ndjson,
        second.path(),
        &options,
    );

    assert_eq!(in_order.written(), 3, "the readable Records");
    assert_eq!(summary.written(), 3, "each Record once");
    assert_eq!(
        fs::read(first.path().join(NDJSON_FILE))?,
        fs::read(second.path().join(NDJSON_FILE))?,
        "the same file"
    );
    Ok(())
}

#[test]
fn the_conflict_policies_apply_to_json_files_and_the_ndjson_file() -> TestResult {
    let bytes = database().build();
    for (form, file) in [
        (RecordForm::Json, "libs/foundry/records/ships/avenger.json"),
        (RecordForm::Ndjson, NDJSON_FILE),
    ] {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(file);
        let options = |policy| RecordExportOptions {
            policy,
            protection: None,
            cancel: None,
            progress: None,
            texts: None,
            tags: None,
        };
        export(&bytes, form, dir.path(), &options(ConflictPolicy::Skip))?;
        let original = fs::read(&path)?;
        fs::write(&path, "changed")?;

        let skip = export(&bytes, form, dir.path(), &options(ConflictPolicy::Skip))?;
        let kept = fs::read(&path)?;
        let changed = export(
            &bytes,
            form,
            dir.path(),
            &options(ConflictPolicy::IfChanged),
        )?;
        let refreshed = fs::read(&path)?;
        let unchanged = export(
            &bytes,
            form,
            dir.path(),
            &options(ConflictPolicy::IfChanged),
        )?;
        fs::write(&path, "changed")?;
        export(
            &bytes,
            form,
            dir.path(),
            &options(ConflictPolicy::Overwrite),
        )?;

        assert_eq!(kept, b"changed", "{form}: skip keeps the file");
        assert!(
            skip.skipped().contains(&file.to_owned()),
            "{form}: skip reports it"
        );
        assert_eq!(
            refreshed, original,
            "{form}: if-changed rewrites a changed file"
        );
        assert!(
            !changed.skipped().contains(&file.to_owned()),
            "{form}: rewritten"
        );
        assert!(
            unchanged.skipped().contains(&file.to_owned()),
            "{form}: unchanged"
        );
        assert_eq!(fs::read(&path)?, original, "{form}: overwrite replaces it");
    }
    Ok(())
}

#[test]
fn a_cancelled_ndjson_export_leaves_no_file() -> TestResult {
    let bytes = database().build();
    let dir = tempfile::tempdir()?;
    let cancel = CancelToken::new();
    cancel.cancel();

    let summary = export(
        &bytes,
        RecordForm::Ndjson,
        dir.path(),
        &RecordExportOptions {
            policy: ConflictPolicy::Skip,
            protection: None,
            cancel: Some(&cancel),
            progress: None,
            texts: None,
            tags: None,
        },
    )?;

    assert_eq!(summary.written(), 0, "nothing written");
    assert_eq!(summary.cancelled(), 4, "every Record");
    assert!(fs::read_dir(dir.path())?.next().is_none(), "no file left");
    Ok(())
}

#[test]
fn records_of_another_dataforge_fail_in_every_form() -> TestResult {
    let bytes = database().build();
    let forge = DataForge::open(&bytes)?;
    let other = DataForge::open(&bytes)?;
    let records: Vec<_> = other.records().take(1).collect();
    for form in RecordForm::ALL {
        let dir = tempfile::tempdir()?;

        let summary = write_record_export(
            &forge,
            &records,
            form,
            dir.path(),
            &RecordExportOptions::default(),
        );

        assert!(
            matches!(
                summary.failures().first().map(RecordFailure::error),
                Some(RecordExportError::OtherDataForge)
            ),
            "{form}: {:?}",
            summary.failures()
        );
        assert_eq!(summary.written(), 0, "{form}: nothing written");
    }
    Ok(())
}

#[test]
fn a_folder_in_the_way_of_the_ndjson_file_is_an_error() -> TestResult {
    let bytes = database().build();
    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join(NDJSON_FILE))?;

    let summary = export(
        &bytes,
        RecordForm::Ndjson,
        dir.path(),
        &RecordExportOptions {
            policy: ConflictPolicy::Overwrite,
            protection: None,
            cancel: None,
            progress: None,
            texts: None,
            tags: None,
        },
    )?;

    assert!(summary.error().is_some(), "the file cannot be written");
    assert!(!summary.is_complete(), "incomplete");
    assert_eq!(summary.written(), 0, "nothing written");
    Ok(())
}

/// Returns every file below `root` with its content, in path order.
///
/// # Errors
///
/// Returns the I/O error of reading the folder.
fn tree(root: &Path) -> Result<String, Box<dyn Error>> {
    let mut files = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut listing = String::new();
    for file in files {
        let relative = file.strip_prefix(root)?.display().to_string();
        listing.push_str("== ");
        listing.push_str(&relative.replace('\\', "/"));
        listing.push('\n');
        listing.push_str(&fs::read_to_string(&file)?);
    }
    Ok(listing)
}
