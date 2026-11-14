// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::{Archive, ArchiveKey, Exclusion, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestAnimation, TestChunkFile, TestDba, TestEntry, test_key};
use serde_json::Value;

use super::support::{self, TestResult};

const DEFINITION: &str = "animation-definition.jsonl";
const FIXTURE_DIRECTORY: &str = "Data/Animations/Fixture/";
const PARAMS: &[u8] = br##"<Params><AnimationList>
  <Animation name="#filepath" path="Animations/Fixture"/>
  <Animation name="both" path="shared.caf"/>
  <Animation name="$TracksDatabase" path="Animations/Fixture/first.dba"/>
  <Animation name="$TracksDatabase" path="Animations/Fixture/second.dba"/>
  <Animation name="only" path="only.caf"/>
  <Animation name="loose" path="loose.caf"/>
  </AnimationList></Params>"##;

#[test]
fn selected_banks_keep_duplicate_physical_ordinals_and_prove_loose_fallback() -> TestResult {
    let temp = tempfile::tempdir()?;
    let archive = archive(temp.path(), Some(test_key()))?;
    let output = temp.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition, MediaKind::Animation],
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, DEFINITION)?;
    let links = link_rows(&rows);
    let [(_, shared), (only_ordinal, only), (_, loose)] = links.as_slice() else {
        return Err("expected three concrete and ordered bindings".into());
    };
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for (_, row) in &links {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    verify_binding(
        shared,
        "ambiguous-bank-match",
        Some(true),
        "2",
        &[("0", "0"), ("1", "0")],
    );
    verify_binding(only, "bank-match", Some(true), "1", &[("0", "2")]);
    verify_binding(loose, "loose-lookup", Some(true), "0", &[]);
    verify_origins(&output.join(index.artifact_base), *only_ordinal)
}

#[test]
fn a_filtered_encrypted_bank_is_unread_and_prevents_a_complete_fallback() -> TestResult {
    let temp = tempfile::tempdir()?;
    let archive = archive(temp.path(), None)?;
    let output = temp.path().join("out");
    let exclusion = Exclusion::parse([format!("{FIXTURE_DIRECTORY}first.dba")])?;
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition, MediaKind::Animation],
        exclude: exclusion,
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, DEFINITION)?;
    let links = link_rows(&rows);
    let [(_, shared), (_, only), (_, loose)] = links.as_slice() else {
        return Err("expected three registered bindings".into());
    };
    verify_binding(shared, "incomplete", Some(false), "1", &[("1", "0")]);
    verify_binding(only, "incomplete", Some(false), "0", &[]);
    verify_binding(loose, "incomplete", Some(false), "0", &[]);
    assert_eq!(
        shared
            .data
            .pointer("/payload/databases/0/state")
            .and_then(Value::as_str),
        Some("not-selected"),
        "explicit exclusion keeps the bank unselected",
    );
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        include: Some(p4kx_extraction::Filter::parse("*.chrparams")?),
        ..ExportOptions::default()
    };
    let output = temp.path().join("metadata");
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, DEFINITION)?;
    assert!(
        link_rows(&rows).is_empty(),
        "definition-only export never obtains semantic bank contents"
    );
    Ok(())
}

#[test]
fn a_damaged_bank_keeps_later_known_ordinals_without_proving_a_complete_binding() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("Data.p4k");
    let params = br##"<Params><AnimationList>
      <Animation name="#filepath" path="Animations/Fixture"/>
      <Animation name="$TracksDatabase" path="Animations/Fixture/partial.dba"/>
      <Animation name="match" path="known.caf"/>
      </AnimationList></Params>"##;
    let dba = partial_database()?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Objects/fixture.chrparams", params))
        .entry(TestEntry::stored(
            format!("{FIXTURE_DIRECTORY}partial.dba"),
            dba,
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let output = temp.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition, MediaKind::Animation],
        diagnostic: true,
        ..ExportOptions::default()
    };
    let ExportOutcome::Incomplete { diagnostic, .. } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("the malformed bank cannot publish a complete generation".into());
    };
    let folder = diagnostic.parent().ok_or("diagnostic folder")?;
    let rows: Vec<MediaRecord> = fs::read_to_string(folder.join(DEFINITION))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let bindings = link_rows(&rows);
    let [(_, row)] = bindings.as_slice() else {
        return Err("known bank declaration retained one binding row".into());
    };
    verify_binding(row, "incomplete", Some(false), "1", &[("0", "1")]);
    Ok(())
}

fn partial_database() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let original = TestDba::new()
        .animation(TestAnimation::new("Animations/Fixture/other.caf", 30))
        .animation(TestAnimation::new("Animations/Fixture/known.caf", 30))
        .animation(TestAnimation::new("Animations/Fixture/tail.caf", 30))
        .bytes();
    let parsed = ChunkFile::parse(original.as_slice())?;
    let (index, chunk) = parsed
        .chunks_of(ivo::DBA_METADATA)
        .next()
        .ok_or("metadata chunk")?;
    let mut bytes = parsed.chunk_data(index).ok_or("metadata bytes")?.to_vec();
    let final_byte = bytes.last_mut().ok_or("final name terminator")?;
    assert_eq!(
        *final_byte, 0,
        "fixture terminator is present before corruption"
    );
    *final_byte = b'x';
    Ok(TestChunkFile::ivo()
        .chunk(ivo::DBA_DATA.id(), 0x900, b"\0\0\0\0bad block")
        .chunk(ivo::DBA_METADATA.id(), chunk.version(), bytes)
        .bytes())
}

fn archive(root: &Path, key: Option<ArchiveKey>) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = root.join("Data.p4k");
    let first = TestDba::new()
        .animation(TestAnimation::new("Animations/Fixture/shared.caf", 30))
        .animation(TestAnimation::new("Animations/Other/only.caf", 30))
        .animation(TestAnimation::new("Data/Animations/Fixture/only.caf", 30))
        .bytes();
    let second = TestDba::new()
        .animation(TestAnimation::new(
            "data\\animations\\fixture\\shared.caf",
            60,
        ))
        .bytes();
    let loose = TestAnimation::new("loose.caf", 30).caf();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Objects/fixture.chrparams", PARAMS))
        .entry(TestEntry::stored(format!("{FIXTURE_DIRECTORY}first.dba"), first).encrypted())
        .entry(TestEntry::stored(
            format!("{FIXTURE_DIRECTORY}second.dba"),
            second,
        ))
        .entry(TestEntry::stored(
            format!("{FIXTURE_DIRECTORY}loose.caf"),
            loose,
        ))
        .write_to(&path)?;
    Ok(Archive::open_with(path, OpenOptions::new().key(key))?)
}

fn link_rows(rows: &[MediaRecord]) -> Vec<(usize, &MediaRecord)> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == "animation-definition-named-dba-binding")
        .collect()
}

fn verify_binding(
    row: &MediaRecord,
    state: &str,
    complete: Option<bool>,
    observed: &str,
    expected: &[(&str, &str)],
) {
    let text = |field| row.data.pointer(field).and_then(Value::as_str);
    assert_eq!(text("/payload/state"), Some(state), "semantic bank result");
    assert_eq!(
        row.data
            .pointer("/payload/candidates_complete")
            .and_then(Value::as_bool),
        complete,
        "candidate completeness"
    );
    assert_eq!(
        text("/payload/observed_candidates"),
        Some(observed),
        "observed candidate count"
    );
    let actual: Vec<_> = row
        .data
        .pointer("/payload/candidates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some((
                item.get("declaration_ordinal")?.as_str()?,
                item.get("clip_ordinal")?.as_str()?,
            ))
        })
        .collect();
    assert_eq!(
        actual, expected,
        "original bank and clip physical order, independent of basename"
    );
}

fn verify_origins(base: &Path, ordinal: usize) -> TestResult {
    let ordinal = ordinal.to_string();
    let origins = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let mut checked = 0;
    for row in origins.lines().map(serde_json::from_str::<Value>) {
        let row = row?;
        if row.pointer("/scope/stream/kind").and_then(Value::as_str) != Some("animation-definition")
            || row.pointer("/scope/ordinal").and_then(Value::as_str) != Some(ordinal.as_str())
        {
            continue;
        }
        let expected = match row.get("field").and_then(Value::as_str) {
            Some(
                "/data/payload/candidates/0/authored_name"
                | "/data/payload/candidates/0/clip_ordinal",
            ) => true,
            Some(
                "/data/payload/candidates/0/entry/entry/path"
                | "/data/payload/databases/0/authored_path",
            ) => false,
            _ => continue,
        };
        assert_eq!(
            row.get("containsEncryptedSource").and_then(Value::as_bool),
            Some(expected),
            "source encryption matches actual field bytes"
        );
        checked += 1;
    }
    assert_eq!(
        checked, 4,
        "parsed bank metadata and directory identity have separate receipts"
    );
    Ok(())
}
