// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Every `p4kx dcb` command over a DataForge file on disk in place of an Archive.

#[path = "dcb_file_origin_guard.rs"]
mod origin_guard;
#[path = "dcb_file_parity.rs"]
mod parity;

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct,
    TestType, TestValue,
};

type TestResult = Result<(), Box<dyn Error>>;

const CART: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;
const TRUCK: u128 = 0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d;

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Vehicle").field("name", TestType::Locale))
        .record(
            TestRecord::new(
                CART,
                "Vehicle",
                "Vehicle.Cart",
                "libs/foundry/records/vehicles/cart.xml",
            )
            .value("name", TestValue::text("@vehicle_Cart")),
        )
        .record(
            TestRecord::new(
                TRUCK,
                "Vehicle",
                "Vehicle.Truck",
                "libs/foundry/records/vehicles/truck.xml",
            )
            .value("name", TestValue::text("@vehicle_Truck")),
        )
}

/// An Archive holding the DataForge and an English `global.ini`, and the same DataForge as a file
/// of its own, extracted the way a user would keep it.
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    archive: PathBuf,
    file: PathBuf,
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().to_path_buf();
    let archive = root.join("Data.p4k");
    let english = GlobalIniBuilder::new()
        .text("vehicle_Cart", "Cart")
        .text("vehicle_Truck", "Truck")
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()))
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            english,
        ))
        .write_to(&archive)?;
    let file = root.join("Game2.dcb");
    fs::write(&file, database().build())?;
    Ok(Fixture {
        _dir: dir,
        root,
        archive,
        file,
    })
}

fn run(args: &[&str], input: &Path, rest: &[&str], code: i32) -> Result<String, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .arg("dcb")
        .args(args)
        .arg(input)
        .args(rest)
        .output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    Ok(format!(
        "{}{}",
        String::from_utf8(output.stdout)?,
        String::from_utf8(output.stderr)?
    ))
}

fn files(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, Box<dyn Error>> {
    let mut found = BTreeMap::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                found.insert(path.strip_prefix(root)?.to_path_buf(), fs::read(&path)?);
            }
        }
    }
    Ok(found)
}

#[test]
fn every_command_reads_a_dataforge_file_like_the_archive() -> TestResult {
    let fixture = fixture()?;
    let commands: [(&[&str], &[&str]); 4] = [
        (&["query"], &["from Vehicle select name"]),
        (&["fields"], &[]),
        (&["refs"], &["--unresolved"]),
        (&["tags"], &[]),
    ];

    for (command, rest) in commands {
        let from_file = run(command, &fixture.file, rest, 0)?;
        let from_archive = run(command, &fixture.archive, rest, 0)?;
        let neutral = |text: &str, path: &Path| {
            text.replace(&path.display().to_string(), "[INPUT]")
                .replace(&path.display().to_string().replace('\\', "\\\\"), "[INPUT]")
        };
        assert_eq!(
            neutral(&from_file, &fixture.file),
            neutral(&from_archive, &fixture.archive),
            "{command:?}"
        );
    }
    Ok(())
}

#[test]
fn info_names_the_file_and_the_version() -> TestResult {
    let fixture = fixture()?;

    let text = run(&["info"], &fixture.file, &[], 0)?;

    assert!(text.contains("File:"), "{text}");
    assert!(text.contains("Game2.dcb"), "{text}");
    assert!(text.contains("Records:         2"), "{text}");
    Ok(())
}

#[test]
fn a_file_is_recognised_by_content_not_by_name() -> TestResult {
    let fixture = fixture()?;
    let renamed = fixture.root.join("kept.bin");
    fs::copy(&fixture.file, &renamed)?;

    let text = run(&["info"], &renamed, &[], 0)?;

    assert!(text.contains("kept.bin"), "{text}");
    Ok(())
}

#[test]
fn the_export_of_a_file_and_of_its_archive_are_byte_identical() -> TestResult {
    let fixture = fixture()?;
    let from_file = fixture.root.join("from-file");
    let from_archive = fixture.root.join("from-archive");

    run(
        &["export"],
        &fixture.file,
        &["--out", &from_file.display().to_string()],
        0,
    )?;
    run(
        &["export"],
        &fixture.archive,
        &["--out", &from_archive.display().to_string()],
        0,
    )?;

    parity::assert_record_export_parity(&from_file, &from_archive, None)
}

#[test]
fn the_combined_export_is_named_after_the_file() -> TestResult {
    let fixture = fixture()?;
    let out = fixture.root.join("combined");

    run(
        &["export"],
        &fixture.file,
        &["--combined", "--out", &out.display().to_string()],
        0,
    )?;

    assert!(out.join("Game2.xml").is_file(), "{:?}", files(&out)?.keys());
    Ok(())
}

#[test]
fn export_of_a_file_without_an_out_folder_makes_an_unnamed_channel_folder() -> TestResult {
    let fixture = fixture()?;

    let output = p4kx_testkit::p4kx()?
        .current_dir(&fixture.root)
        .args(["dcb", "export"])
        .arg(&fixture.file)
        .output()?;

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let made = fs::read_dir(&fixture.root)?
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("SC-EXPORT-UnknownChannel-UnknownBuild-")
        });
    assert!(made, "the default folder is named without Channel or build");
    Ok(())
}

#[test]
fn lang_on_a_file_needs_an_archive_for_the_texts() -> TestResult {
    let fixture = fixture()?;
    let out = fixture.root.join("out");

    let message = run(
        &["export"],
        &fixture.file,
        &["--lang", "en", "--out", &out.display().to_string()],
        2,
    )?;

    assert!(message.contains("needs --archive"), "{message}");
    assert!(!out.exists(), "nothing is written");
    Ok(())
}

#[test]
fn lang_on_a_file_takes_the_texts_from_the_given_archive() -> TestResult {
    let fixture = fixture()?;
    let from_file = fixture.root.join("from-file");
    let from_archive = fixture.root.join("from-archive");

    run(
        &["export"],
        &fixture.file,
        &[
            "--lang",
            "en",
            "--archive",
            &fixture.archive.display().to_string(),
            "--out",
            &from_file.display().to_string(),
        ],
        0,
    )?;
    run(
        &["export"],
        &fixture.archive,
        &["--lang", "en", "--out", &from_archive.display().to_string()],
        0,
    )?;

    parity::assert_record_export_parity(&from_file, &from_archive, Some("en"))
}

#[test]
fn archive_option_is_refused_when_the_input_is_an_archive() -> TestResult {
    let fixture = fixture()?;
    let message = run(
        &["export"],
        &fixture.archive,
        &[
            "--lang",
            "en",
            "--archive",
            &fixture.archive.display().to_string(),
            "--out",
            &fixture.root.join("out").display().to_string(),
        ],
        2,
    )?;

    assert!(
        message.contains("the input is an Archive already"),
        "{message}"
    );
    Ok(())
}

#[test]
fn a_truncated_file_exits_with_code_5() -> TestResult {
    let fixture = fixture()?;
    let whole = database().build();
    fs::write(&fixture.file, &whole[..200])?;

    for command in ["info", "tags", "fields"] {
        let message = run(&[command], &fixture.file, &[], 5)?;
        assert!(message.contains("truncated"), "{command}: {message}");
    }
    Ok(())
}

#[test]
fn a_file_of_an_unsupported_version_exits_with_code_5() -> TestResult {
    let fixture = fixture()?;
    fs::write(&fixture.file, database().version(5).build())?;

    let message = run(&["info"], &fixture.file, &[], 5)?;

    assert!(
        message.contains("DataForge version 5 is not supported"),
        "{message}"
    );
    Ok(())
}

#[cfg(target_pointer_width = "64")]
#[test]
fn a_large_declaration_with_short_data_exits_as_a_read_error() -> TestResult {
    let fixture = fixture()?;
    let oversize = fixture.root.join("Oversize.p4k");
    ArchiveBuilder::new()
        .entry(
            TestEntry::stored("Data\\Game2.dcb", database().build())
                .with_uncompressed_size((2 << 30) + 1),
        )
        .write_to(&oversize)?;

    let message = run(&["info"], &oversize, &[], 3)?;

    assert!(
        message.contains("cannot read the DataForge Entry"),
        "{message}"
    );
    Ok(())
}

#[test]
fn a_file_that_is_neither_an_archive_nor_a_dataforge_exits_with_code_3() -> TestResult {
    let fixture = fixture()?;
    fs::write(&fixture.file, "this is a text file, long enough to be read")?;

    run(&["info"], &fixture.file, &[], 3)?;
    Ok(())
}

#[test]
fn executable_zero_filled_and_tiny_files_exit_with_code_3() -> TestResult {
    let fixture = fixture()?;
    let mut executable = b"MZ\x90\x00\x03\x00\x00\x00\x04\x00\x00\x00".to_vec();
    executable.resize(1 << 20, 0x41);
    let inputs: [(&str, Vec<u8>); 3] = [
        ("an executable", executable),
        ("a zero-filled file", vec![0; 4096]),
        ("a file under 8 bytes", vec![0, 0, 0, 0, 6]),
    ];

    for (what, bytes) in inputs {
        fs::write(&fixture.file, bytes)?;
        let message = run(&["info"], &fixture.file, &[], 3)?;
        assert!(!message.contains("DataForge version"), "{what}: {message}");
    }
    Ok(())
}

#[test]
fn a_missing_file_exits_with_code_3() -> TestResult {
    let fixture = fixture()?;

    run(&["info"], &fixture.root.join("missing.dcb"), &[], 3)?;
    Ok(())
}
