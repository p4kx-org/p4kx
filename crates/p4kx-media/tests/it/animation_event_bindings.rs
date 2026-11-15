// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, OpenOptions};
use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestModel, test_key};
use serde_json::Value;

use super::support::{self, TestResult};

const MODEL: &str = "Data/Objects/rig.chr";
const PARAMS: &str = "Data/Objects/rig.chrparams";
const BANK: &str = "Data/Animations/ordered.animevents";
const STREAM: &str = "animation-definition.jsonl";

#[test]
fn event_bindings_pair_exact_authored_database_and_duplicated_timeline_sites() -> TestResult {
    let temp = tempfile::tempdir()?;
    let archive = fixture(temp.path())?;
    for scoped in [false, true] {
        let output = temp.path().join(if scoped { "scoped" } else { "global" });
        let options = ExportOptions {
            kinds: vec![MediaKind::AnimationDefinition],
            animation_model: scoped.then(|| MODEL.to_owned()),
            ..ExportOptions::default()
        };
        let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
        let rows = support::records(&output, &index, STREAM)?;
        let bindings = verify_bindings(&archive, &rows)?;
        verify_origins(&output.join(&index.artifact_base), bindings)?;
        if scoped {
            assert_eq!(
                index.source_entries.written, 2,
                "only the authored definition closure"
            );
        }
    }
    Ok(())
}

fn fixture(root: &Path) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = root.join("Data.p4k");
    let events = br#"<anim_event_list>
      <animation name="Animations/One/same.caf"/>
      <animation name="data\animations\ONE\SAME.CAF"/>
      <animation name="Animations/Two/same.caf"/>
    </anim_event_list>"#;
    let parameters = br##"<Params>
      <AnimationList>
        <Animation name="#filepath" path="Animations/One"/>
        <Animation name="same" path="same.caf"/>
        <Animation name="same" path="same.caf"/>
        <Animation name="$AnimEventDatabase" path="Animations/ordered.animevents"/>
        <Animation name="$AnimEventDatabase" path="Animations/ordered.animevents"/>
      </AnimationList>
      <AnimationList><Animation name="unlinked" path="Animations/One/same.caf"/></AnimationList>
    </Params>"##;
    ArchiveBuilder::new()
        .entry(TestEntry::stored(BANK, events).encrypted())
        .entry(TestEntry::stored(
            "Data/Animations/unreferenced.animevents",
            events,
        ))
        .entry(TestEntry::stored(MODEL, TestModel::cube().cgf()))
        .entry(TestEntry::stored(PARAMS, parameters))
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn verify_bindings(
    archive: &Archive,
    rows: &[MediaRecord],
) -> Result<usize, Box<dyn std::error::Error>> {
    let valid = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let sites: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == "animation-definition-event-binding")
        .collect();
    assert_eq!(
        sites.len(),
        8,
        "two aliases, two database declarations, two exact timelines"
    );
    let entry = |path: &str| {
        archive
            .entries()
            .iter()
            .find(|entry| entry.path() == path)
            .map(|entry| entry.index().to_string())
            .ok_or("fixture entry")
    };
    let params = entry(PARAMS)?;
    let bank = entry(BANK)?;
    let mut aliases = BTreeSet::new();
    let mut databases = BTreeSet::new();
    let mut timelines = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for (_, row) in &sites {
        valid
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
        let field = |name| {
            row.data
                .pointer(&format!("/payload/{name}"))
                .and_then(Value::as_str)
        };
        assert_eq!(
            field("parameter_entry_index"),
            Some(params.as_str()),
            "original parameters"
        );
        assert_eq!(
            field("event_database_entry_index"),
            Some(bank.as_str()),
            "authored physical database"
        );
        aliases.insert(field("alias_element_index"));
        databases.insert(field("database_element_index"));
        timelines.insert(field("timeline_element_index"));
        identities.insert(&row.subresource.indices);
    }
    assert_eq!(
        (aliases.len(), databases.len(), timelines.len()),
        (2, 2, 2),
        "every duplicate retains its own original coordinate"
    );
    assert_eq!(
        identities.len(),
        sites.len(),
        "equal authoring remains separate"
    );
    Ok(sites.first().ok_or("event binding row")?.0)
}

fn verify_origins(base: &Path, ordinal: usize) -> TestResult {
    let text = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let ordinal = ordinal.to_string();
    let mut checked = 0;
    for line in text.lines() {
        let row: Value = serde_json::from_str(line)?;
        if row.pointer("/scope/stream/kind").and_then(Value::as_str) != Some("animation-definition")
            || row.pointer("/scope/ordinal").and_then(Value::as_str) != Some(ordinal.as_str())
        {
            continue;
        }
        let encrypted = match row.get("field").and_then(Value::as_str) {
            Some("/data/payload/timeline_element_index") => true,
            Some("/data/payload/alias_element_index") => false,
            _ => continue,
        };
        assert_eq!(
            row.get("containsEncryptedSource").and_then(Value::as_bool),
            Some(encrypted),
            "exact authoring and timeline evidence stay independent",
        );
        checked += 1;
    }
    assert_eq!(checked, 2, "each selector uses its own original input");
    Ok(())
}
