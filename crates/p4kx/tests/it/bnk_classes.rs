// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx bnk list` shows the class of every media item.

use std::error::Error;
use std::fs;

use p4kx_testkit::{TestBank, TestCodec, TestWem};

type TestResult = Result<(), Box<dyn Error>>;

fn bank() -> Vec<u8> {
    TestBank::new(0x00C1_A550)
        .wem(
            100,
            TestWem::encoded(TestCodec::Vorbis, 2, 48_000, 72_000).bytes(),
        )
        .wem(200, b"MThd\0\0\0\x06\0\x01\0\x01\x01\xE0".to_vec())
        .wem(300, b"PLUG\0\0\0\0".to_vec())
        .wem(400, b"ADM3\0\0\0\0".to_vec())
        .bytes()
}

#[test]
fn list_names_the_class_of_every_item_in_text_and_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("Classes.bnk");
    fs::write(&file, bank())?;

    let text = p4kx_testkit::p4kx()?
        .args(["bnk", "list"])
        .arg(&file)
        .output()?;
    assert_eq!(text.status.code(), Some(0), "{text:?}");
    let text = String::from_utf8(text.stdout)?;
    let rows: Vec<Vec<&str>> = text
        .lines()
        .filter(|line| line.trim_start().starts_with(|c: char| c.is_ascii_digit()))
        .map(|line| line.split_whitespace().take(2).collect())
        .collect();
    assert_eq!(
        rows,
        [
            vec!["100", "WEM"],
            vec!["200", "MIDI"],
            vec!["300", "plug-in"],
            vec!["400", "unknown"]
        ],
        "{text}"
    );

    let json = p4kx_testkit::p4kx()?
        .args(["--json", "bnk", "list"])
        .arg(&file)
        .output()?;
    let json: serde_json::Value = serde_json::from_slice(&json.stdout)?;
    let classes: Vec<&str> = json["wems"]
        .as_array()
        .ok_or("wems")?
        .iter()
        .filter_map(|wem| wem["class"].as_str())
        .collect();
    assert_eq!(classes, ["wem", "midi", "impulse", "unknown"]);
    Ok(())
}

#[test]
fn extract_as_wem_writes_midi_media_with_the_mid_extension() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("Classes.bnk");
    fs::write(&file, bank())?;
    let out = dir.path().join("out");

    p4kx_testkit::p4kx()?
        .args(["bnk", "extract"])
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);

    let mut names: Vec<String> = fs::read_dir(&out)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()?;
    names.sort();
    assert_eq!(
        names,
        [
            "100.wem",
            "100.wem.p4kx-origin.jsonl",
            "200.mid",
            "200.mid.p4kx-origin.jsonl",
            "300.wem",
            "300.wem.p4kx-origin.jsonl",
            "400.wem",
            "400.wem.p4kx-origin.jsonl",
        ],
        "raw media and their matched companion filenames",
    );
    Ok(())
}
