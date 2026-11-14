// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx actionmap export` end to end, over a synthetic default profile.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, GlobalIniBuilder, TestEntry};
use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn Error>>;

#[path = "actionmap_origins.rs"]
mod origins;

const PROFILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<profile version="1">
  <actionmap name="seat_general" version="1" UILabel="@ui_CGSeatGeneral">
    <action name="v_eject" activationMode="press" keyboard="ralt+y" gamepad=" " joystick="button4" UILabel="@ui_CIEject"/>
    <action name="prone" activationMode="press" keyboard="lctrl" UILabel="@ui_CIProne">
      <gamepad activationMode="delayed_press" input="b"/>
    </action>
    <action name="v_nolabel" activationMode="tap" keyboard="=cmd" UILabel="@ui_Missing"/>
  </actionmap>
</profile>"#;

fn profile() -> TestEntry {
    TestEntry::zstd("Data\\Libs\\Config\\defaultProfile.xml", PROFILE)
}

fn english() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\english\\global.ini",
        GlobalIniBuilder::new()
            .text("ui_CGSeatGeneral", "Seat General")
            .text("ui_CIEject", "Eject")
            .text("ui_CIProne", "Prone, \"flat\"")
            .build(),
    )
}

fn german() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\german_(germany)\\global.ini",
        GlobalIniBuilder::new()
            .text("ui_CGSeatGeneral", "Sitz allgemein")
            .text("ui_CIEject", "Ausstoßen")
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
    archive(dir, vec![profile(), english(), german()])
}

fn stdout_of(args: &[&std::ffi::OsStr], code: i32) -> Result<String, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?.args(args).output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

fn export(archive: &Path, extra: &[&str], code: i32) -> Result<String, Box<dyn Error>> {
    let mut args: Vec<&std::ffi::OsStr> = vec!["actionmap".as_ref(), "export".as_ref()];
    args.push(archive.as_os_str());
    args.extend(extra.iter().map(|argument| std::ffi::OsStr::new(*argument)));
    stdout_of(&args, code)
}

#[test]
fn csv_has_one_row_per_action_with_labels_and_bindings_per_device() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = standard(dir.path())?;
    let csv = export(&path, &["--format", "csv"], 0)?;
    assert_eq!(
        csv,
        "\u{feff}action_map,action_map_label,action,label,activation_mode,keyboard,mouse,gamepad,joystick\r\n\
         seat_general,Seat General,v_eject,Eject,press,ralt+y,,,button4\r\n\
         seat_general,Seat General,prone,\"Prone, \"\"flat\"\"\",press,lctrl,,b (delayed_press),\r\n\
         seat_general,Seat General,v_nolabel,,tap,'=cmd,,,\r\n"
    );
    Ok(())
}

#[test]
fn json_lists_every_binding_with_its_device_and_mode() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = standard(dir.path())?;
    let json: serde_json::Value = serde_json::from_str(&export(&path, &[], 0)?)?;
    assert_eq!(json["language"], "en");
    let map = &json["action_maps"][0];
    assert_eq!(map["name"], "seat_general");
    assert_eq!(map["label_key"], "@ui_CGSeatGeneral");
    assert_eq!(map["label"], "Seat General");
    let prone = &map["actions"][1];
    assert_eq!(prone["name"], "prone");
    assert_eq!(prone["label"], "Prone, \"flat\"");
    assert_eq!(
        prone["bindings"],
        serde_json::json!([
            {"device": "keyboard", "input": "lctrl", "activation_mode": "press"},
            {"device": "gamepad", "input": "b", "activation_mode": "delayed_press"},
        ])
    );
    assert_eq!(map["actions"][2]["label"], serde_json::Value::Null);
    assert_eq!(map["actions"][2]["label_key"], "@ui_Missing");
    Ok(())
}

#[test]
fn language_picks_the_label_text() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = standard(dir.path())?;
    let json: serde_json::Value = serde_json::from_str(&export(&path, &["--language", "de"], 0)?)?;
    assert_eq!(json["language"], "de");
    assert_eq!(json["action_maps"][0]["label"], "Sitz allgemein");
    assert_eq!(json["action_maps"][0]["actions"][0]["label"], "Ausstoßen");
    assert_eq!(
        json["action_maps"][0]["actions"][1]["label"],
        serde_json::Value::Null
    );
    Ok(())
}

#[test]
fn out_writes_the_file_and_reports_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = standard(dir.path())?;
    let out = dir.path().join("keys.csv");
    let report = export(
        &path,
        &["--format", "csv", "--out", out.to_str().ok_or("path")?],
        0,
    )?;
    assert!(report.contains("3 actions in 1 action maps, unresolved labels in en: 1"));
    assert!(std::fs::read_to_string(&out)?.contains("v_eject"));
    Ok(())
}

#[test]
fn a_language_the_archive_lacks_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = standard(dir.path())?;
    p4kx_testkit::p4kx()?
        .args(["actionmap", "export"])
        .arg(&path)
        .args(["--language", "fr"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no global.ini for the language"));
    Ok(())
}

#[test]
fn an_archive_without_the_profile_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = archive(dir.path(), vec![english()])?;
    p4kx_testkit::p4kx()?
        .args(["actionmap", "export"])
        .arg(&path)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("defaultProfile.xml"));
    Ok(())
}

#[test]
fn a_profile_that_is_no_xml_is_an_archive_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = archive(
        dir.path(),
        vec![
            TestEntry::zstd(
                "Data\\Libs\\Config\\defaultProfile.xml",
                "<profile><actionmap>",
            ),
            english(),
        ],
    )?;
    p4kx_testkit::p4kx()?
        .args(["actionmap", "export"])
        .arg(&path)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("defaultProfile.xml"));
    Ok(())
}

#[test]
fn an_encrypted_profile_without_a_key_exits_with_7() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = archive(
        dir.path(),
        vec![
            TestEntry::deflated("Data\\Libs\\Config\\defaultProfile.xml", PROFILE).encrypted(),
            english(),
        ],
    )?;
    p4kx_testkit::p4kx()?
        .env_remove("P4KX_ARCHIVE_KEY")
        .env(
            "P4KX_KEY_FILE",
            dir.path().join("profile").join("archive.key"),
        )
        .args(["actionmap", "export"])
        .arg(&path)
        .assert()
        .code(7)
        .stderr(predicate::str::contains("P4KX_ARCHIVE_KEY"));
    Ok(())
}

#[test]
fn csv_raw_writes_the_exact_texts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = standard(dir.path())?;
    let csv = export(&path, &["--format", "csv", "--csv-raw"], 0)?;
    assert!(csv.starts_with('\u{feff}'));
    assert!(csv.contains(",v_nolabel,,tap,=cmd,"), "{csv}");
    assert!(!csv.contains("'=cmd"));
    Ok(())
}

#[test]
fn csv_raw_with_json_is_invalid_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = standard(dir.path())?;
    p4kx_testkit::p4kx()?
        .args(["actionmap", "export"])
        .arg(&path)
        .arg("--csv-raw")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--csv-raw needs --format csv"));
    Ok(())
}
