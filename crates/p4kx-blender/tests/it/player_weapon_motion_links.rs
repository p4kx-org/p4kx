// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_assembly::BodyVariant;
use p4kx_blender::{player_bundle_record_schema, write_player_source_bundle};
use p4kx_extraction::CancelToken;
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::TestResult;

#[test]
fn weapon_motion_keeps_exact_loadout_candidates_and_scoped_tag_occurrences() -> TestResult {
    let extra = [
        TestEntry::stored(
            "Data/Animations/test/root-tags.xml",
            br#"<TagDefinition version="2"><Tags>
                <Tag name="ONE"/><Tag name="Two"/><Tag name="two"/>
            </Tags></TagDefinition>"#,
        ),
        TestEntry::stored(
            "Data/Animations/test/tags.xml",
            br#"<TagDefinition version="2"><Tags>
                <Tag name="One"/>
            </Tags></TagDefinition>"#,
        )
        .encrypted(),
    ];
    let (_fixture, archive) = super::fixture_with(
        r#"<SubADBs><SubADB File="Animations/test/owned.adb"/></SubADBs>"#,
        extra.into_iter().chain([TestEntry::stored(
            "Data/Animations/test/owned.adb",
            br#"
            <AnimDB TagDef="Animations/test/tags.xml">
                <FragmentList><Walk><Fragment Tags="one+one+two"/></Walk></FragmentList>
            </AnimDB>"#,
        )]),
    )?;
    let directory = tempfile::tempdir()?;
    let output = directory.path().join("bundle");
    write_player_source_bundle(&archive, &[BodyVariant::Male], &output, &CancelToken::new())?;
    let rows = fs::read_to_string(output.join("records.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    verify_loadout_candidates(&rows)?;
    verify_postings(&rows)?;
    let schema = jsonschema::validator_for(&player_bundle_record_schema())?;
    for row in rows.iter().filter(|row| {
        row["type"]
            .as_str()
            .is_some_and(|kind| kind.starts_with("weaponMotion"))
    }) {
        schema.validate(row).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn verify_loadout_candidates(rows: &[Value]) -> TestResult {
    let sources: Vec<_> = rows
        .iter()
        .filter(|row| row["type"] == "weaponMotionSource")
        .collect();
    let [one, two] = sources.as_slice() else {
        return Err("two physical weapon records".into());
    };
    assert_ne!(
        one.pointer("/value/record/value"),
        two.pointer("/value/record/value"),
        "display name cannot merge physical candidate records"
    );
    assert_eq!(
        one.pointer("/value/tags/value"),
        Some(&json!("One one Two")),
        "full original Tags spelling"
    );
    assert_eq!(
        one.pointer("/value/tags/containsEncryptedSource"),
        Some(&json!(true)),
        "actual source encryption applies"
    );
    let entries: Vec<_> = rows
        .iter()
        .filter(|row| row["type"] == "weaponMotionLoadout")
        .collect();
    assert_eq!(
        entries.len(),
        2,
        "same candidates available at both original item occurrences"
    );
    for entry in entries {
        assert_eq!(
            entry.pointer("/value/ambiguousTarget/value"),
            Some(&json!(true)),
            "no arbitrary same-name candidate"
        );
        assert_eq!(
            entry
                .pointer("/value/weapons")
                .and_then(Value::as_array)
                .map(|slots| slots
                    .iter()
                    .filter_map(|slot| slot["value"].as_u64())
                    .collect::<Vec<_>>()),
            Some(vec![0, 1]),
            "both physical weapon candidates kept in original order"
        );
    }
    Ok(())
}

fn verify_postings(rows: &[Value]) -> TestResult {
    let postings: Vec<_> = rows
        .iter()
        .filter(|row| row["type"] == "weaponMotionTag")
        .collect();
    assert_eq!(
        postings.len(),
        3,
        "two root spellings and one different-scope posting"
    );
    let competing = postings
        .iter()
        .find(|row| row.pointer("/value/basis/value") == Some(&json!("competing")))
        .ok_or("competing definition basis")?;
    assert_eq!(
        competing
            .pointer("/value/weaponTokens")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "duplicate records contribute distinct tokens"
    );
    let choices: Vec<_> = postings
        .iter()
        .filter(|row| row.pointer("/value/basis/value") == Some(&json!("unique")))
        .collect();
    let [root, branch] = choices.as_slice() else {
        return Err("a scoped choice in each physical ADB".into());
    };
    assert_ne!(
        root.pointer("/value/scope/value"),
        branch.pointer("/value/scope/value"),
        "same spelling is not a cross-scope posting"
    );
    assert_eq!(
        branch
            .pointer("/value/weaponTokens")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "repeated authored weapon spelling remains ordered"
    );
    assert_eq!(
        branch
            .pointer("/value/optionTokens")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "repeated ADB token retains each ordinal"
    );
    verify_posting_origins(branch);
    Ok(())
}

fn verify_posting_origins(branch: &Value) {
    for (field, encrypted) in [
        ("/value/basis/containsEncryptedSource", true),
        (
            "/value/weaponTokens/0/spelling/containsEncryptedSource",
            true,
        ),
        (
            "/value/optionTokens/0/spelling/containsEncryptedSource",
            false,
        ),
    ] {
        assert_eq!(
            branch.pointer(field),
            Some(&json!(encrypted)),
            "actual source of {field}"
        );
    }
}
