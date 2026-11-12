// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase,
    wwise_short_id,
};
use serde_json::{Value, json};

use super::{EMBEDDED, Fixture, PLAY, TestResult, extract, pcm};

const MULTIPLE: &str = "Play_Choice";
const SINGLE: &str = "Play_Only_Choice";
const TXTP_PATH: &str = "Play_Choice.txtp";
const TXTP_FILE: &str = "/txtp/files/0";
const SELECTION: &str = "select-first-alternative";
const FLAGS: [&str; 3] = ["--format", "wav", "--txtp"];

#[test]
fn rendered_first_choice_is_reported_in_the_header_and_json() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, MULTIPLE, &FLAGS, 0)?;
    let generated = fs::read_to_string(fixture.out.join(TXTP_PATH))?;
    assert_eq!(
        report.pointer("/txtp/files/0/approximations"),
        Some(&json!([SELECTION])),
        "the report names the selection the writer actually emits",
    );
    assert!(
        generated
            .lines()
            .take(2)
            .any(|line| line == "# approximation: select-first-alternative"),
        "the emitted TXTP header reports the same code: {generated}",
    );
    assert!(
        generated
            .lines()
            .any(|line| line.trim_start() == "group = -R2>1"),
        "the existing vgmstream selector syntax is preserved: {generated}",
    );

    let flags = ["--format", "wav", "--txtp", "--conflict", "if-changed"];
    let identical = extract(&fixture, MULTIPLE, &flags, 0)?;
    let file = identical
        .pointer(TXTP_FILE)
        .ok_or("retained TXT file report")?;
    assert_eq!(
        file.get("kept"),
        Some(&json!(true)),
        "byte-identical TXTP is kept"
    );
    assert_eq!(
        file.get("approximations"),
        Some(&json!([SELECTION])),
        "a byte-identical TXTP still has verified generated diagnostics",
    );
    assert_eq!(
        fs::read_to_string(fixture.out.join(TXTP_PATH))?,
        generated,
        "the verified retained artifact carries the same diagnostics",
    );
    Ok(())
}

#[test]
fn one_child_does_not_add_a_selection_approximation() -> TestResult {
    let fixture = fixture()?;
    let report = extract(&fixture, SINGLE, &FLAGS, 0)?;
    assert_eq!(
        report.pointer("/txtp/files/0/approximations"),
        Some(&json!([])),
        "the collapsed single child emits no TXTP choice",
    );
    assert_eq!(
        fs::read_to_string(fixture.out.join("Play_Only_Choice.txtp"))?,
        "# P4KX: event Play_Only_Choice of trigger Play_Only_Choice\nPlay_Only_Choice.wav\n",
        "single-child TXTP stays byte-for-byte compatible",
    );
    Ok(())
}

#[test]
fn unchecked_skip_keeps_existing_bytes_and_reports_unknown_diagnostics() -> TestResult {
    let fixture = fixture()?;
    fs::create_dir_all(&fixture.out)?;
    let existing = fixture.out.join(TXTP_PATH);
    let sentinel = b"# externally retained preview\n";
    fs::write(&existing, sentinel)?;

    let flags = ["--format", "wav", "--txtp", "--conflict", "skip"];
    let report = extract(&fixture, MULTIPLE, &flags, 0)?;
    let file = report.pointer(TXTP_FILE).ok_or("kept TXT file report")?;
    assert_eq!(
        file.get("kept"),
        Some(&json!(true)),
        "skip reports the retained file"
    );
    assert_eq!(
        file.get("approximations"),
        Some(&Value::Null),
        "the generated preview does not attest to arbitrary retained bytes",
    );
    assert_eq!(
        fs::read(existing)?,
        sentinel,
        "skip preserves the user file"
    );
    Ok(())
}

fn fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let out = dir.path().join("out");
    let definitions = TestAtl::new("SC_Audio")
        .trigger(MULTIPLE, &[MULTIPLE])
        .trigger(SINGLE, &[SINGLE]);
    let bank = TestBank::new(wwise_short_id("Choices"))
        .version(154)
        .wem(100, pcm())
        .wem(101, pcm())
        .object(TestHircObject::event(wwise_short_id(MULTIPLE), &[1]))
        .object(TestHircObject::event(wwise_short_id(SINGLE), &[2]))
        .object(TestHircObject::action(1, PLAY, 10))
        .object(TestHircObject::action(2, PLAY, 11))
        .object(TestHircObject::container(
            TestContainerKind::Random,
            10,
            &[20, 21],
            TestNodeBase::Plain,
        ))
        .object(TestHircObject::container(
            TestContainerKind::Random,
            11,
            &[20],
            TestNodeBase::Plain,
        ))
        .object(TestHircObject::sound(20, 100, EMBEDDED, 0))
        .object(TestHircObject::sound(21, 101, EMBEDDED, 0))
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data/Libs/GameAudio/SC_Audio.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::stored("Data/Sounds/wwise/Choices.bnk", bank))
        .write_to(&archive)?;
    Ok(Fixture {
        _dir: dir,
        archive,
        out,
    })
}
