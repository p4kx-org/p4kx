// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_assembly::BodyVariant;
use p4kx_blender::write_player_source_bundle;
use p4kx_extraction::CancelToken;
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::{Value, json};

use super::TestResult;

fn original_sections() -> TestEntry {
    let root = TestElement::new("AnimDB")
        .attribute("TagDef", "Animations/test/root-tags.xml")
        .attribute("FragDef", "Animations/test/fragments.xml")
        .child(
            TestElement::new("SubADBs")
                .child(
                    TestElement::new("SubADB")
                        .attribute("File", "Animations/test/child.adb")
                        .attribute("Tags", "Global+unknown")
                        .child(TestElement::new("Unknown"))
                        .child(TestElement::new("FragmentID").attribute("Name", "walk"))
                        .child(TestElement::new("FragmentID").attribute("Name", "Walk"))
                        .child(TestElement::new("FragmentID").attribute("Name", "notDeclared"))
                        .child(
                            TestElement::new("FragmentID")
                                .attribute("Name", "Walk")
                                .attribute("Name", "Idle"),
                        )
                        .child(TestElement::new("FragmentID")),
                )
                .child(
                    TestElement::new("SubADB")
                        .attribute("File", "DATA/ANIMATIONS/TEST/CHILD.ADB")
                        .attribute("Tags", "Global")
                        .child(TestElement::new("FragmentID").attribute("Name", "")),
                ),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    TestEntry::stored("Data/Animations/test/player.adb", root)
}

fn entries() -> [TestEntry; 3] {
    [
        TestEntry::stored("Data/Animations/test/root-tags.xml",
            br#"<TagDefinition version="2"><Tags><Tag name="GLOBAL"/></Tags></TagDefinition>"#),
        TestEntry::stored("Data/Animations/test/child-tags.xml",
            br#"<TagDefinition version="2"><Tags><Tag name="CHILD"/></Tags></TagDefinition>"#).encrypted(),
        TestEntry::stored("Data/Animations/test/child.adb",
            br#"<AnimDB TagDef="Animations/test/child-tags.xml" FragDef="Animations/test/fragments.xml"><SubADBs>
                <SubADB File="Animations/test/player.adb" Tags="Child"><FragmentID Name="Idle"/></SubADB>
            </SubADBs></AnimDB>"#),
    ]
}

#[test]
fn branch_selectors_keep_original_edge_and_parent_definition_scope() -> TestResult {
    let [controller, _, fragments] = super::motion_option_assertions::entries("");
    let (_fixture, archive) =
        super::fixture_with_motion([controller, original_sections(), fragments], entries())?;
    let directory = tempfile::tempdir()?;
    let output = directory.path().join("bundle");
    write_player_source_bundle(&archive, &[BodyVariant::Male], &output, &CancelToken::new())?;
    let rows = fs::read_to_string(output.join("records.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let selected: Vec<_> = rows
        .iter()
        .filter(|row| row["type"] == "adbBranchSelectors")
        .collect();
    let [first, alias, child] = selected.as_slice() else {
        return Err("two root edges and one cyclic leaf edge".into());
    };
    verify_parent_scope(first, alias, child);
    verify_fragment_occurrences(first)?;
    verify_physical_links(&rows, first, alias)?;
    Ok(())
}

fn verify_parent_scope(first: &Value, alias: &Value, child: &Value) {
    assert_eq!(
        first["document"], alias["document"],
        "two branches authored on same root"
    );
    assert_ne!(
        first["index"], alias["index"],
        "same destination retains distinct owner occurrences"
    );
    assert_ne!(
        first["document"], child["document"],
        "child keeps its original physical ADB"
    );
    assert_eq!(
        first.pointer("/globalTags/tokens/0/matches/0/name/value"),
        Some(&json!("GLOBAL")),
        "root and alias match only parent global tags"
    );
    assert_eq!(
        first.pointer("/globalTags/tokens/1/status"),
        Some(&json!("missing")),
        "unmatched parent tag explicit"
    );
    assert_eq!(
        child.pointer("/globalTags/tokens/0/matches/0/name/value"),
        Some(&json!("CHILD")),
        "cyclic branch uses its own originating ADB scope"
    );
    assert_eq!(
        child.pointer("/globalTags/tokens/0/containsEncryptedSource"),
        Some(&json!(true)),
        "actual child tag definition contributes encrypted bytes"
    );
}

fn verify_fragment_occurrences(row: &Value) -> TestResult {
    let definitions = row["fragmentDefinitions"]
        .as_array()
        .ok_or("direct FragmentID rows")?;
    let [first, repeated, missing, invalid, omitted] = definitions.as_slice() else {
        return Err("every direct authored selector retained".into());
    };
    let expected = [1, 2, 3, 4, 5];
    for (occurrence, ordinal) in definitions.iter().zip(expected) {
        assert_eq!(
            occurrence["ordinal"],
            json!(ordinal),
            "ordinal includes unknown intervening element"
        );
    }
    assert_eq!(
        first.pointer("/lookup/status"),
        Some(&json!("resolved")),
        "case-folded name present"
    );
    assert_eq!(
        first.pointer("/lookup/matches"),
        repeated.pointer("/lookup/matches"),
        "separate original occurrences share one physical declaration"
    );
    assert_eq!(
        first.pointer("/lookup/containsEncryptedSource"),
        Some(&json!(true)),
        "fragment name uses encrypted FragDef"
    );
    assert_eq!(
        missing.pointer("/lookup/status"),
        Some(&json!("missing")),
        "complete FragDef can prove a miss"
    );
    assert_eq!(
        invalid.pointer("/name/state"),
        Some(&json!("invalid")),
        "multiple attributes remain invalid"
    );
    assert_eq!(
        invalid
            .pointer("/name/occurrences")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "both duplicate values and positions survive"
    );
    assert_eq!(
        omitted.pointer("/lookup"),
        Some(&Value::Null),
        "no invented lookup for absent Name"
    );
    Ok(())
}

fn verify_physical_links(rows: &[Value], first: &Value, alias: &Value) -> TestResult {
    let target = |selector: &Value| {
        rows.iter()
            .find(|row| {
                row["type"] == "databaseLink"
                    && row["branch"] == 0
                    && row["index"] == selector["index"]
            })
            .and_then(|row| row.pointer("/value/target/value/data"))
    };
    let first_target = target(first).ok_or("root link has exact physical target")?;
    assert_eq!(
        Some(first_target),
        target(alias),
        "alias still points to the same independently traversed branch"
    );
    Ok(())
}
