// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_assembly::BodyVariant;
use p4kx_blender::{PlayerSource, write_player_source_bundle};
use p4kx_extraction::CancelToken;
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::TestResult;

fn originals() -> [TestEntry; 3] {
    [
        TestEntry::stored(
            "Data/Animations/test/controller.xml",
            br#"<ControllerDef><Tags filename="Animations/test/unused.xml"/><Fragments filename="Animations/test/fragments.xml"/></ControllerDef>"#,
        ),
        TestEntry::stored(
            "Data/Animations/test/player.adb",
            r#"<AnimDB TagDef="Animations/test/global.xml" FragDef="Animations/test/fragments.xml"><FragmentList>
                <WALK><Fragment Tags="same+duP++imported+absent+á" FragTags="same+missing"/><Fragment Tags=""/></WALK>
                <Partial><Fragment FragTags="near+miss"/></Partial>
                <Repeat><Fragment FragTags="same"/></Repeat>
                <NoScope><Fragment FragTags="same"/></NoScope>
                <Unknown><Fragment FragTags="same"/></Unknown>
            </FragmentList></AnimDB>"#.as_bytes(),
        ),
        TestEntry::stored(
            "Data/Animations/test/fragments.xml",
            br#"<TagDefinition version="2"><Tags><Tag name="walk" subTagDef="Animations/test/local.xml"/>
                <Tag name="Partial" subTagDef="Animations/test/partial.xml"/>
                <Tag name="Repeat" subTagDef="Animations/test/local.xml"/><Tag name="repeat" subTagDef="Animations/test/partial.xml"/>
                <Tag name="NoScope"/></Tags></TagDefinition>"#,
        )
        .encrypted(),
    ]
}

fn definitions() -> [TestEntry; 5] {
    [
        TestEntry::stored(
            "Data/Animations/test/global.xml",
            br#"<TagDefinition version="2"><Imports><Import filename="Animations/test/common.xml"/>
                <Import filename="data/animations/test/COMMON.XML"/></Imports>
                <Tags><Tag name="Same"/><Group name="plain"><Tag name="dup"/></Group></Tags></TagDefinition>"#,
        ),
        TestEntry::stored(
            "Data/Animations/test/common.xml",
            br#"<TagDefinition version="2"><Imports><Import filename="Animations/test/global.xml"/></Imports>
                <Tags><Tag name="Imported"/><Tag name="DUP"/></Tags></TagDefinition>"#,
        )
        .encrypted(),
        TestEntry::stored(
            "Data/Animations/test/local.xml",
            br#"<TagDefinition version="2"><Tags><Tag name="SAME"/></Tags></TagDefinition>"#,
        ),
        TestEntry::stored(
            "Data/Animations/test/partial.xml",
            br#"<TagDefinition version="2"><Imports><Import filename="Animations/test/missing.xml"/>
                <Import filename="Animations/test/unreadable.xml"/></Imports><Tags><Tag name="Near"/></Tags></TagDefinition>"#,
        ),
        TestEntry::stored("Data/Animations/test/unreadable.xml", br"<TagDefinition/>")
            .encrypted()
            .with_crc32c(0),
    ]
}

#[test]
fn authored_adb_tags_bind_distinct_scopes_and_physical_candidates() -> TestResult {
    let (_fixture, archive) = super::fixture_with_motion(originals(), definitions())?;
    let directory = tempfile::tempdir()?;
    let output = directory.path().join("tags");
    let cancel = CancelToken::new();
    let bodies = [BodyVariant::Male];
    let old = serde_json::to_value(PlayerSource::read(&archive, bodies[0], &cancel)?)?;
    write_player_source_bundle(&archive, &bodies, &output, &cancel)?;
    let rows: Vec<Value> = fs::read_to_string(output.join("records.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let selected: Vec<_> = rows
        .iter()
        .filter(|row| row["type"] == "adbTagSelectors")
        .collect();
    let [full, empty, partial, duplicate, no_scope, unmatched] = selected.as_slice() else {
        return Err("expected six authored tag option rows".into());
    };
    verify_full(full, &rows)?;
    assert_eq!(
        empty.pointer("/global/tokens"),
        Some(&json!([])),
        "explicit empty attribute has no tokens"
    );
    assert_eq!(
        empty.pointer("/global/attribute/occurrences/0/value/value"),
        Some(&json!("")),
        "the original empty string remains attached to its attribute"
    );
    assert_eq!(
        empty.pointer("/fragment/attribute/state"),
        Some(&json!("absent")),
        "the other scope stays authored-absent"
    );
    verify_partial(partial, &rows)?;
    verify_absent_or_ambiguous(duplicate, no_scope, unmatched);
    let after = serde_json::to_value(PlayerSource::read(&archive, bodies[0], &cancel)?)?;
    assert_eq!(
        after, old,
        "a read-only lookup outputs the same legacy projection"
    );
    Ok(())
}

fn verify_full(row: &Value, rows: &[Value]) -> TestResult {
    let tokens = row
        .pointer("/global/tokens")
        .and_then(Value::as_array)
        .ok_or("global tokens")?;
    let [same, two, empty, imported, absent, non_ascii] = tokens.as_slice() else {
        return Err("each authored token has an output".into());
    };
    verify_mixed_origin(same);
    assert_eq!(
        two["status"],
        json!("ambiguous"),
        "a name collision does not choose either candidate"
    );
    assert_eq!(
        two["matches"].as_array().map(Vec::len),
        Some(2),
        "distinct physical definitions remain separate"
    );
    assert_eq!(
        empty["status"],
        json!("emptyToken"),
        "an adjacent separator produces an explicit invalid token"
    );
    assert_eq!(
        empty["spelling"],
        json!(""),
        "the empty token has its authored empty spelling"
    );
    assert_eq!(
        imported["matches"].as_array().map(Vec::len),
        Some(1),
        "alias imports and cycles do not duplicate physical declarations"
    );
    assert_eq!(
        absent["status"],
        json!("missing"),
        "a complete global scope can prove a missing tag"
    );
    assert_eq!(
        non_ascii["status"],
        json!("unsupportedSpelling"),
        "no unsupported case folding is applied"
    );
    verify_fragment(row, rows, same);
    Ok(())
}

fn verify_mixed_origin(same: &Value) {
    assert_eq!(
        same.pointer("/matches/0/name/value"),
        Some(&json!("Same")),
        "a normalized match keeps the original definition spelling"
    );
    assert_eq!(
        same.pointer("/matches/0/name/containsEncryptedSource"),
        Some(&json!(false)),
        "the candidate name is sourced to its own plain definition"
    );
    assert_eq!(
        same["containsEncryptedSource"],
        json!(true),
        "the comparison also uses the encrypted imported definitions"
    );
    assert_eq!(
        same["sourceSetComplete"],
        json!(true),
        "the cyclic but fully observed scope is complete"
    );
}

fn verify_fragment(row: &Value, rows: &[Value], same: &Value) {
    assert_eq!(
        row.pointer("/fragment/tokens/0/matches/0/name/value"),
        Some(&json!("SAME")),
        "the fragment tag binds to the scoped spelling"
    );
    assert_ne!(
        row.pointer("/fragment/tokens/0/matches/0/document"),
        same.pointer("/matches/0/document"),
        "equal spellings in different scopes have different physical owners"
    );
    assert_eq!(
        row.pointer("/fragment/tokens/1/status"),
        Some(&json!("missing")),
        "scoped candidates do not fall back to global tags"
    );
    assert!(
        rows.iter()
            .any(|candidate| candidate["type"] == "adbFragment"
                && candidate["document"] == row["document"]
                && candidate["index"] == row["index"]),
        "the selectors row refers to the exact option in the physical ADB"
    );
}

fn verify_partial(row: &Value, rows: &[Value]) -> TestResult {
    let tokens = row
        .pointer("/fragment/tokens")
        .and_then(Value::as_array)
        .ok_or("partial tokens")?;
    let [near, miss] = tokens.as_slice() else {
        return Err("two original tokens".into());
    };
    assert_eq!(
        near["status"],
        json!("incomplete"),
        "a partial scope cannot establish uniqueness"
    );
    assert_eq!(
        near["matches"].as_array().map(Vec::len),
        Some(1),
        "the observed candidate survives a partial scope"
    );
    assert_eq!(
        miss["status"],
        json!("incomplete"),
        "an unresolved import prevents proving absence"
    );
    assert_eq!(
        miss["sourceSetComplete"],
        json!(false),
        "unavailable imports make comparison evidence incomplete"
    );
    let scope = row
        .pointer("/fragment/fragmentOwner/tagScope")
        .ok_or("scope ref")?;
    let scope = rows
        .iter()
        .find(|row| row["type"] == "adbTagScope" && row["index"] == *scope)
        .ok_or("scope row")?;
    assert_eq!(
        scope.pointer("/value/containsEncryptedSource"),
        Some(&json!(true)),
        "a scoped edge records its encrypted FragmentID owner"
    );
    assert_eq!(
        scope.pointer("/value/sourceSetComplete"),
        Some(&json!(false)),
        "the shared scope row contains the incomplete origin"
    );
    Ok(())
}

fn verify_absent_or_ambiguous(duplicate: &Value, no_scope: &Value, unmatched: &Value) {
    assert_eq!(
        duplicate.pointer("/fragment/tokens/0/status"),
        Some(&json!("ambiguous")),
        "multiple physical FragmentIDs prevent resolving a local tag"
    );
    assert_eq!(
        duplicate
            .pointer("/fragment/fragmentOwner/lookup/matches")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "both conflicting FragmentID declarations survive"
    );
    assert_eq!(
        no_scope.pointer("/fragment/tokens/0/status"),
        Some(&json!("noFragmentTagDefinition")),
        "a present FragmentID can author no tag definition"
    );
    assert_eq!(
        unmatched.pointer("/fragment/tokens/0/status"),
        Some(&json!("noFragmentDefinition")),
        "a complete search can prove no FragmentID declaration"
    );
}
