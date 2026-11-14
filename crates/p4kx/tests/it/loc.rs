// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx loc` end to end, over synthetic `global.ini` files in several encodings.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use p4kx_testkit::{ArchiveBuilder, GlobalIniBuilder, TestEntry};
use predicates::prelude::predicate;

#[path = "loc_json_origin.rs"]
mod json_origin;
#[path = "loc_table_origins.rs"]
mod table_origins;

use json_origin::payload_snapshot;

type TestResult = Result<(), Box<dyn Error>>;

fn english() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\english\\global.ini",
        GlobalIniBuilder::new()
            .text("item_NameRifle", "Rifle")
            .text("item_DescRifle", "Shoots.\\nWell, \"very\" well.")
            .text("item_OnlyEnglish", "Only in English")
            .text("item_EmptyGerman", "Filled")
            .text("LOC_EMPTY", "")
            .build(),
    )
}

fn german_utf16() -> TestEntry {
    let text = "item_NameRifle=Gewehr\r\nitem_DescRifle=Schie\u{df}t.\\nGut.\r\nitem_EmptyGerman=\r\nno equals line\r\n";
    let mut bytes = b"\xff\xfe".to_vec();
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    TestEntry::zstd("Data\\Localization\\german_(germany)\\global.ini", bytes)
}

fn japanese() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\japanese_(japan)\\global.ini",
        GlobalIniBuilder::new()
            .text("item_NameRifle", "\u{30e9}\u{30a4}\u{30d5}\u{30eb}")
            .build(),
    )
}

fn archive(dir: &Path, entries: Vec<TestEntry>) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(path)
}

fn standard(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    archive(dir, vec![english(), german_utf16(), japanese()])
}

fn p4kx() -> Result<Command, Box<dyn Error>> {
    Ok(p4kx_testkit::p4kx()?)
}

fn stdout_of(command: &mut Command, code: i32) -> Result<String, Box<dyn Error>> {
    let output = command.output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

#[test]
fn languages_lists_the_languages_the_archive_holds() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = standard(dir.path())?;
    let text = stdout_of(p4kx()?.args(["loc", "languages"]).arg(&archive), 0)?;
    insta::assert_snapshot!("loc_languages", text.replace('\\', "/"));
    let json = stdout_of(
        p4kx()?.args(["--json", "loc", "languages"]).arg(&archive),
        0,
    )?;
    let value: serde_json::Value = serde_json::from_str(&json)?;
    let codes: Vec<&str> = value["languages"]
        .as_array()
        .ok_or("languages")?
        .iter()
        .filter_map(|language| language["code"].as_str())
        .collect();
    assert_eq!(codes, ["en", "de", "ja"], "in the order of the languages");
    Ok(())
}

#[test]
fn get_prints_the_text_of_a_key_with_or_without_at_ignoring_case() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = standard(dir.path())?;
    for key in ["item_NameRifle", "@ITEM_namerifle"] {
        let text = stdout_of(p4kx()?.args(["loc", "get"]).arg(&archive).arg(key), 0)?;
        assert_eq!(text, "Rifle\n", "English by default for {key}");
    }
    let german = stdout_of(
        p4kx()?
            .args(["loc", "get"])
            .arg(&archive)
            .args(["item_DescRifle", "--lang", "de"]),
        0,
    )?;
    assert_eq!(
        german, "Schie\u{df}t.\nGut.\n",
        "UTF-16 file, line break decoded"
    );
    let japanese = stdout_of(
        p4kx()?.args(["loc", "get"]).arg(&archive).args([
            "item_NameRifle",
            "--lang",
            "japanese_(japan)",
        ]),
        0,
    )?;
    assert_eq!(
        japanese, "\u{30e9}\u{30a4}\u{30d5}\u{30eb}\n",
        "folder name accepted"
    );
    Ok(())
}

#[test]
fn get_falls_back_when_the_language_has_no_text() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = standard(dir.path())?;
    for (key, expected) in [
        ("item_OnlyEnglish", "Only in English\n"),
        ("item_EmptyGerman", "Filled\n"),
    ] {
        let text = stdout_of(
            p4kx()?.args(["loc", "get"]).arg(&archive).arg(key).args([
                "--lang",
                "de",
                "--fallback",
                "en",
            ]),
            0,
        )?;
        assert_eq!(text, expected, "{key} comes from the fallback");
    }
    let json = stdout_of(
        p4kx()?.args(["--json", "loc", "get"]).arg(&archive).args([
            "item_OnlyEnglish",
            "--lang",
            "de",
            "--fallback",
            "en",
        ]),
        0,
    )?;
    insta::assert_snapshot!("loc_get_fallback_json", payload_snapshot(&json)?);
    let own = stdout_of(
        p4kx()?.args(["--json", "loc", "get"]).arg(&archive).args([
            "item_NameRifle",
            "--lang",
            "de",
            "--fallback",
            "en",
        ]),
        0,
    )?;
    insta::assert_snapshot!("loc_get_found_json", payload_snapshot(&own)?);
    Ok(())
}

#[test]
fn get_of_a_missing_key_exits_with_2() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = standard(dir.path())?;
    p4kx()?
        .args(["loc", "get"])
        .arg(&archive)
        .args(["item_OnlyEnglish", "--lang", "de"])
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::contains(
            "item_OnlyEnglish has no text in de",
        ));
    let json = stdout_of(
        p4kx()?.args(["--json", "loc", "get"]).arg(&archive).args([
            "nowhere",
            "--lang",
            "de",
            "--fallback",
            "en",
        ]),
        2,
    )?;
    insta::assert_snapshot!("loc_get_missing_json", payload_snapshot(&json)?);
    Ok(())
}

#[test]
fn get_of_a_placeholder_key_has_no_text_but_succeeds() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = standard(dir.path())?;
    let json = stdout_of(
        p4kx()?
            .args(["--json", "loc", "get"])
            .arg(&archive)
            .arg("@LOC_EMPTY"),
        0,
    )?;
    insta::assert_snapshot!("loc_get_placeholder_json", payload_snapshot(&json)?);
    Ok(())
}

#[test]
fn get_escapes_control_characters_in_plain_output_only() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(
        dir.path(),
        vec![TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            GlobalIniBuilder::new()
                .text("item_Escape", "red\u{1b}[31m text \u{202e}end\u{61c}")
                .build(),
        )],
    )?;
    let plain = stdout_of(
        p4kx()?
            .args(["loc", "get"])
            .arg(&archive)
            .arg("item_Escape"),
        0,
    )?;
    assert_eq!(
        plain, "red\\u{1b}[31m text \\u{202e}end\\u{61c}\n",
        "no raw escape or bidi character reaches the terminal"
    );
    let json = stdout_of(
        p4kx()?
            .args(["--json", "loc", "get"])
            .arg(&archive)
            .arg("item_Escape"),
        0,
    )?;
    let value: serde_json::Value = serde_json::from_str(&json)?;
    assert_eq!(
        value["text"], "red\u{1b}[31m text \u{202e}end\u{61c}",
        "JSON keeps the text"
    );
    Ok(())
}

#[test]
fn a_language_the_archive_lacks_or_does_not_know_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = standard(dir.path())?;
    p4kx()?
        .args(["loc", "get"])
        .arg(&archive)
        .args(["item_NameRifle", "--lang", "fr"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "no global.ini for the language fr",
        ));
    p4kx()?
        .args(["loc", "get"])
        .arg(&archive)
        .args(["item_NameRifle", "--lang", "klingon"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unknown language"));
    p4kx()?
        .args(["loc", "get"])
        .arg(&archive)
        .args(["item_NameRifle", "--fallback", "it"])
        .assert()
        .code(2);
    Ok(())
}

#[test]
fn export_writes_every_key_with_its_texts_as_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = standard(dir.path())?;
    let out = dir.path().join("texts.json");
    let summary = stdout_of(
        p4kx()?
            .args(["loc", "export"])
            .arg(&archive)
            .arg("--out")
            .arg(&out),
        0,
    )?;
    assert!(summary.contains("5 keys in 3 languages"), "{summary}");
    let text = fs::read_to_string(&out)?;
    serde_json::from_str::<serde_json::Value>(&text)?;
    insta::assert_snapshot!("loc_export_json", text);
    Ok(())
}

#[test]
fn export_writes_csv_for_the_chosen_languages_to_stdout() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = standard(dir.path())?;
    let csv = stdout_of(
        p4kx()?
            .args(["loc", "export"])
            .arg(&archive)
            .args(["--format", "csv", "--lang", "de,en"]),
        0,
    )?;
    assert_eq!(
        csv,
        concat!(
            "\u{feff}key,de,en\r\n",
            "item_DescRifle,\"Schie\u{df}t.\nGut.\",\"Shoots.\nWell, \"\"very\"\" well.\"\r\n",
            "item_EmptyGerman,,Filled\r\n",
            "item_NameRifle,Gewehr,Rifle\r\n",
            "item_OnlyEnglish,,Only in English\r\n",
            "LOC_EMPTY,,\r\n",
        ),
        "BOM, CRLF and quoting"
    );
    let japanese_only = stdout_of(
        p4kx()?
            .args(["loc", "export"])
            .arg(&archive)
            .args(["--format", "csv", "--lang", "ja"]),
        0,
    )?;
    assert_eq!(
        japanese_only, "\u{feff}key,ja\r\nitem_NameRifle,\u{30e9}\u{30a4}\u{30d5}\u{30eb}\r\n",
        "keys without a text in the chosen languages are left out"
    );
    Ok(())
}

#[test]
fn export_csv_raw_keeps_the_byte_order_mark_and_needs_the_csv_format() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = standard(dir.path())?;
    let args = ["--format", "csv", "--csv-raw", "--lang", "ja"];
    let csv = stdout_of(p4kx()?.args(["loc", "export"]).arg(&archive).args(args), 0)?;
    assert_eq!(
        csv, "\u{feff}key,ja\r\nitem_NameRifle,\u{30e9}\u{30a4}\u{30d5}\u{30eb}\r\n",
        "raw CSV"
    );
    let mut command = p4kx()?;
    let output = command
        .args(["loc", "export"])
        .arg(&archive)
        .arg("--csv-raw")
        .output()?;
    assert_eq!(output.status.code(), Some(2), "needs --format csv");
    Ok(())
}

#[test]
fn export_json_summary_names_the_columns() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = standard(dir.path())?;
    let out = dir.path().join("texts.csv");
    let summary = stdout_of(
        p4kx()?
            .args(["--json", "loc", "export"])
            .arg(&archive)
            .arg("--out")
            .arg(&out)
            .args(["--format", "csv", "--lang", "en"]),
        0,
    )?;
    let value: serde_json::Value = serde_json::from_str(&summary)?;
    assert_eq!(value["keys"], 5, "keys");
    assert_eq!(value["format"], "csv", "format");
    assert_eq!(value["languages"], serde_json::json!(["en"]), "languages");
    Ok(())
}

#[test]
fn export_of_an_archive_without_any_global_ini_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(
        dir.path(),
        vec![TestEntry::stored("Data\\a.txt", b"x".to_vec())],
    )?;
    p4kx()?
        .args(["loc", "export"])
        .arg(&archive)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no global.ini for any language"));
    Ok(())
}
