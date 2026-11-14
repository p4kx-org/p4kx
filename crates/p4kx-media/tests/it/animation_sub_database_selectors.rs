// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::{Archive, Exclusion};
use p4kx_media::{ExportOptions, Identity, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry, test_key,
};
use serde_json::Value;

use super::support;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const PRIMARY: &str = "Data/Animations/root.adb";
const SECOND: &str = "Data/Animations/other.adb";
const FRAGMENTS: &str = "Data/Animations/fragments.xml";
const SHARED: &str = "Data/Animations/shared.xml";
const TAGS: &str = "Data/Animations/tags.xml";
const ROOT: &[u8] = br#"<AnimDB TagDef="Animations/tags.xml" FragDef="Animations/fragments.xml">
  <SubADBs>
    <SubADB File="Animations/child.adb" Tags="Ready++ready+missing">
      <FragmentID Name="Run"/><FragmentID Name=""/><FragmentID/><FragmentID Name="Repeat"/>
    </SubADB>
    <SubADB File="Animations/child.adb" Tags="Ready"><FragmentID Name="Run"/></SubADB>
  </SubADBs></AnimDB>"#;

#[test]
fn sub_database_selectors_use_parent_definitions_and_keep_repeated_branch_occurrences() -> TestResult
{
    let directory = tempfile::tempdir()?;
    let archive = fixture(directory.path())?;
    let (rows, base) = project(&archive, directory.path(), false)?;
    let primary = roles(&rows, "sub-database-token", PRIMARY);
    assert_eq!(
        token_states(&primary),
        [
            ("Ready", "resolved"),
            ("", "invalid-token"),
            ("ready", "resolved"),
            ("missing", "missing"),
            ("Run", "resolved"),
            ("Repeat", "ambiguous"),
            ("Ready", "resolved"),
            ("Run", "resolved")
        ],
        "owning scope resolves tokens and original branch order preserves repetitions"
    );
    let other = roles(&rows, "sub-database-token", SECOND);
    assert_eq!(
        token_states(&other),
        [("Ready", "missing"), ("Run", "missing")],
        "same physical child does not supply the owning ADB's definition selectors"
    );
    verify_candidates(&rows, &base, &primary)?;
    verify_selection_shape(&rows)
}

#[test]
fn sub_database_selectors_distinguish_unselected_definition_from_missing_token() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = fixture(directory.path())?;
    let (rows, _) = project(&archive, directory.path(), true)?;
    let primary = roles(&rows, "sub-database-token", PRIMARY);
    assert_eq!(
        token_states(&primary).first(),
        Some(&("Ready", "incomplete")),
        "an excluded existing definition cannot prove a token missing"
    );
    let selectors = roles(&rows, "sub-database-selector", PRIMARY);
    assert_eq!(
        selectors
            .first()
            .and_then(|row| row.data.pointer("/payload/scope"))
            .and_then(Value::as_str),
        Some("not-selected"),
        "existing excluded target has separate state"
    );
    assert!(
        roles(&rows, "sub-database-candidate", PRIMARY)
            .iter()
            .all(|row| row
                .data
                .pointer("/payload/target/entry/path")
                .and_then(Value::as_str)
                != Some(TAGS)),
        "no candidate used bytes from an excluded definition"
    );
    Ok(())
}

fn verify_candidates(rows: &[MediaRecord], base: &Path, tokens: &[&MediaRecord]) -> TestResult {
    let candidates = roles(rows, "sub-database-candidate", PRIMARY);
    let run = candidates
        .iter()
        .find(|candidate| {
            candidate
                .data
                .pointer("/payload/name")
                .and_then(Value::as_str)
                == Some("rUn")
        })
        .ok_or("exact physical fragment candidate")?;
    let shared = candidates
        .iter()
        .find(|candidate| {
            candidate
                .data
                .pointer("/payload/target/entry/path")
                .and_then(Value::as_str)
                == Some(SHARED)
        })
        .ok_or("exact imported definition candidate")?;
    assert_eq!(
        run.data
            .pointer("/payload/target/entry/path")
            .and_then(Value::as_str),
        Some(FRAGMENTS),
        "fragment comes from owning FragDef"
    );
    assert_eq!(
        run.data
            .pointer("/payload/name_position/type")
            .and_then(Value::as_str),
        Some("cry-xml-attribute"),
        "CryXML attribute position is retained"
    );
    assert_eq!(
        shared.data.pointer("/payload/name").and_then(Value::as_str),
        Some("READY"),
        "original definition spelling survives"
    );
    assert_eq!(
        candidates
            .iter()
            .filter(
                |row| row.data.pointer("/payload/name").and_then(Value::as_str) == Some("READY")
            )
            .count(),
        3,
        "shared import is visited once per scope, each branch token remains distinct"
    );
    let token = *tokens.first().ok_or("first authored tag token")?;
    verify_receipt(rows, base, token, "/data/payload/spelling", false)?;
    verify_receipt(rows, base, token, "/data/payload/resolution", true)?;
    verify_receipt(rows, base, shared, "/data/payload/name", true)
}

fn verify_selection_shape(rows: &[MediaRecord]) -> TestResult {
    let selectors = roles(rows, "sub-database-selector", PRIMARY);
    assert_eq!(
        selectors
            .iter()
            .map(|row| row.data.pointer("/payload/state").and_then(Value::as_str))
            .collect::<Vec<_>>(),
        [
            Some("present"),
            Some("present"),
            Some("empty"),
            Some("missing"),
            Some("present"),
            Some("present"),
            Some("present")
        ],
        "authored absent and empty child definitions are distinct"
    );
    let mut tags = selectors
        .iter()
        .filter(|row| row.data.pointer("/payload/kind").and_then(Value::as_str) == Some("tags"));
    let first = tags.next().ok_or("first branch")?;
    let second = tags.next().ok_or("second branch")?;
    assert_ne!(
        first.data.pointer("/payload/branch"),
        second.data.pointer("/payload/branch"),
        "each branch to the same physical child stays independently positioned"
    );
    let duplicate = roles(
        rows,
        "sub-database-selector",
        "Data/Animations/repeated.adb",
    );
    assert_eq!(
        duplicate
            .first()
            .and_then(|row| row.data.pointer("/payload/state"))
            .and_then(Value::as_str),
        Some("repeated"),
        "binary duplicate attributes are preserved by the declaration rows"
    );
    assert!(
        roles(rows, "sub-database-token", "Data/Animations/repeated.adb").is_empty(),
        "ambiguous repeated attribute does not invent a joint token stream"
    );
    Ok(())
}

fn fixture(directory: &Path) -> TestResult<Archive> {
    let path = directory.join("scopes.p4k");
    let fragments = TestElement::new("TagDefinition")
        .child(
            TestElement::new("Tags")
                .child(TestElement::new("Tag").attribute("name", "rUn"))
                .child(TestElement::new("Tag").attribute("name", "Repeat"))
                .child(TestElement::new("Tag").attribute("name", "repeat")),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let repeated = TestElement::new("AnimDB")
        .attribute("TagDef", "Animations/tags.xml")
        .child(
            TestElement::new("SubADBs").child(
                TestElement::new("SubADB")
                    .attribute("Tags", "Ready")
                    .attribute("Tags", "missing"),
            ),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    ArchiveBuilder::new()
        .entry(TestEntry::stored(PRIMARY, ROOT))
        .entry(TestEntry::stored(SECOND, br#"<AnimDB TagDef="Animations/empty.xml" FragDef="Animations/empty.xml">
            <SubADBs><SubADB File="Animations/child.adb" Tags="Ready"><FragmentID Name="Run"/></SubADB></SubADBs>
            </AnimDB>"#))
        .entry(TestEntry::stored("Data/Animations/child.adb", b"<AnimDB/>"))
        .entry(TestEntry::stored("Data/Animations/repeated.adb", repeated))
        .entry(TestEntry::stored(TAGS, br#"<TagDefinition><Imports>
            <Import filename="Animations/shared.xml"/><Import filename="Data/Animations/SHARED.XML"/>
            </Imports></TagDefinition>"#))
        .entry(TestEntry::stored(SHARED, br#"<TagDefinition><Imports><Import filename="Animations/tags.xml"/></Imports>
            <Tags><Group name="test"><Tag name="READY"/></Group></Tags></TagDefinition>"#).encrypted())
        .entry(TestEntry::stored(FRAGMENTS, fragments).encrypted())
        .entry(TestEntry::stored("Data/Animations/empty.xml", b"<TagDefinition/>"))
        .write_to(&path)?;
    Ok(Archive::open_with_key(path, Some(test_key()))?)
}

fn project(
    archive: &Archive,
    directory: &Path,
    exclude: bool,
) -> TestResult<(Vec<MediaRecord>, PathBuf)> {
    let output = directory.join("output");
    fs::create_dir(&output)?;
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        exclude: if exclude {
            Exclusion::parse([TAGS])?
        } else {
            Exclusion::default()
        },
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "animation-definition.jsonl")?;
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in &rows {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    Ok((rows, output.join(index.artifact_base)))
}

fn roles<'a>(rows: &'a [MediaRecord], role: &str, path: &str) -> Vec<&'a MediaRecord> {
    rows.iter().filter(|row|
        row.data.get("record_type").and_then(Value::as_str) == Some(role)
        && matches!(&row.source.identity, Identity::Entry { entry, .. } if entry.path == path)
    ).collect()
}

fn token_states<'a>(rows: &[&'a MediaRecord]) -> Vec<(&'a str, &'a str)> {
    rows.iter()
        .map(|row| {
            (
                row.data
                    .pointer("/payload/spelling")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                row.data
                    .pointer("/payload/resolution")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn verify_receipt(
    rows: &[MediaRecord],
    base: &Path,
    target: &MediaRecord,
    field: &str,
    encrypted: bool,
) -> TestResult {
    let ordinal = rows
        .iter()
        .position(|row| std::ptr::eq(row, target))
        .ok_or("physical receipt row")?
        .to_string();
    let receipts = fs::read_to_string(base.join("field-origins.jsonl"))?;
    for line in receipts.lines() {
        let row: Value = serde_json::from_str(line)?;
        if row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(&ordinal)
            && row.pointer("/scope/stream/kind").and_then(Value::as_str)
                == Some("animation-definition")
            && row.get("field").and_then(Value::as_str) == Some(field)
        {
            assert_eq!(
                row.get("containsEncryptedSource").and_then(Value::as_bool),
                Some(encrypted),
                "per-field encryption derives from the sources that field uses"
            );
            assert_eq!(
                row.get("sourceSetComplete").and_then(Value::as_bool),
                Some(true),
                "fully proven field"
            );
            assert!(
                row.get("sourceUsesRefs")
                    .and_then(Value::as_array)
                    .is_some_and(|refs| !refs.is_empty()),
                "direct source use references"
            );
            return Ok(());
        }
    }
    Err("scoped field receipt missing".into())
}
