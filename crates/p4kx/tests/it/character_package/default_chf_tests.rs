// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{
    HEAD_VERTICES, MALE_BODY_TYPE, MALE_RIG_HASH, TestChf, TestEntry, TestItemPort, armour_entries,
    head_entries, head_rig,
};
use serde_json::Value;

use super::{TestResult, archive, chf_fixture, manifest, package};

const MALE_DEFAULT: &str = "Data\\Libs\\CharacterCustomizer\\MasculineDefault.chf";
const FEMALE_DEFAULT: &str = "Data/Libs/CharacterCustomizer/femininedefault.chf";

fn default_chf_fixture() -> Result<Vec<u8>, Box<dyn Error>> {
    let suit = "00000000-0000-0000-0000-000000000001"
        .parse::<p4kx_extraction::Guid>()?
        .stored();
    Ok(TestChf {
        body_type: MALE_BODY_TYPE,
        ports: Some(TestItemPort {
            port: p4kx_assembly::character::lowered_hash("body_itemport"),
            item: [0; 16],
            tail_count: 0,
            children: vec![TestItemPort {
                port: p4kx_assembly::character::lowered_hash("torso_itemport"),
                item: suit,
                tail_count: 0,
                children: Vec::new(),
            }],
        }),
        materials: Vec::new(),
        ..chf_fixture()
    }
    .to_bytes())
}

#[test]
fn the_explicit_default_chf_supplies_the_head_without_equipping_its_outfit() -> TestResult {
    let dir = tempfile::tempdir()?;
    let rig = head_rig(HEAD_VERTICES, MALE_RIG_HASH).build();
    let mut entries = head_entries(Some(rig));
    let entry_index = entries.len();
    entries.push(TestEntry::zstd(MALE_DEFAULT, default_chf_fixture()?).encrypted());
    let archive = archive(dir.path(), entries)?;
    let options = [
        "--body",
        "male",
        "--materials",
        "colours",
        "--no-expressions",
    ];

    for (name, additions, head, suit) in [
        ("body", [].as_slice(), false, false),
        ("head", ["--default-chf"].as_slice(), true, false),
        (
            "worn",
            ["--default-chf", "--piece", "TEST_Suit"].as_slice(),
            true,
            true,
        ),
    ] {
        let out = dir.path().join(name);
        let args: Vec<&str> = options
            .into_iter()
            .chain(additions.iter().copied())
            .collect();
        let run = package(&archive, &out, &args, true)?;
        assert_eq!(run.code, Some(0), "{name}: {}", run.stderr);
        let report: Value = serde_json::from_str(&run.stdout)?;
        let parts = report["parts"].as_array().ok_or("worn parts")?;
        let contains = |name: &str| parts.iter().any(|part| part["name"] == name);
        assert!(contains("body"), "the selected body stays in {name}");
        assert_eq!(contains("head"), head, "head selection for {name}");
        assert_eq!(contains("teeth"), head, "head items for {name}");
        assert_eq!(
            contains("TEST_Suit"),
            suit,
            "clothing follows the requested pieces"
        );
        assert!(report["chf"].is_null(), "no disk CHF was selected");
        let manifest = manifest(&out)?;
        let source = &manifest["source"]["character"];
        assert_default_chf_source(&report, source, head, entry_index);
    }
    Ok(())
}

fn assert_default_chf_source(report: &Value, source: &Value, head: bool, entry_index: usize) {
    if head {
        assert_eq!(
            report.pointer("/default_chf/path").and_then(Value::as_str),
            Some(MALE_DEFAULT),
            "actual Entry path"
        );
        assert_eq!(
            report
                .pointer("/default_chf/entry_index")
                .and_then(Value::as_u64),
            Some(entry_index as u64),
            "physical Entry index"
        );
        assert_eq!(
            source.get("chf").and_then(Value::as_str),
            Some("MasculineDefault.chf"),
            "manifest keeps the selected basename"
        );
    } else {
        assert!(
            report.get("default_chf").is_none(),
            "bare mode did not select the default"
        );
        assert!(source.get("chf").is_none(), "the bare package has no CHF");
    }
}

#[test]
fn an_unusable_default_chf_keeps_its_failure_and_writes_nothing() -> TestResult {
    let good = default_chf_fixture()?;
    let malformed = TestEntry::stored(MALE_DEFAULT, b"invalid CHF".to_vec());
    let unreadable = TestEntry::stored(MALE_DEFAULT, good.clone()).with_crc32c(0);
    for (entries, body, code, message) in [
        (vec![], "male", 2, "is missing"),
        (
            vec![
                TestEntry::stored(MALE_DEFAULT, good.clone()),
                TestEntry::stored(MALE_DEFAULT.to_ascii_lowercase(), good.clone()),
            ],
            "male",
            2,
            "matches 2 physical Entries",
        ),
        (vec![malformed], "male", 3, "as CHF"),
        (
            vec![unreadable],
            "male",
            3,
            "cannot read the default CHF Entry",
        ),
        (
            vec![TestEntry::stored(FEMALE_DEFAULT, good)],
            "female",
            2,
            "uses the male rig for the selected female body",
        ),
    ] {
        let dir = tempfile::tempdir()?;
        let mut archive_entries = armour_entries();
        archive_entries.extend(entries);
        let archive = archive(dir.path(), archive_entries)?;
        let out = dir.path().join("refused");
        let run = package(&archive, &out, &["--default-chf", "--body", body], false)?;
        assert_eq!(run.code, Some(code), "{}", run.stderr);
        assert!(run.stderr.contains(message), "{}", run.stderr);
        assert!(!out.exists(), "a rejected CHF cannot publish a package");
    }
    Ok(())
}

#[test]
fn the_default_chf_requires_an_explicit_body_and_conflicts_with_a_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path(), armour_entries())?;
    let out = dir.path().join("refused");
    for (args, message) in [
        (["--default-chf"].as_slice(), "--body"),
        (
            ["--default-chf", "--chf", "hero.chf", "--body", "male"].as_slice(),
            "cannot be used with",
        ),
    ] {
        let run = package(&archive, &out, args, false)?;
        assert_eq!(run.code, Some(2), "{}", run.stderr);
        assert!(run.stderr.contains(message), "{}", run.stderr);
        assert!(!out.exists(), "a refused request cannot publish a package");
    }
    Ok(())
}
