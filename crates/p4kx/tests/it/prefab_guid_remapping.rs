// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_assembly::PrefabSet;
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::TestResult;

const OWNER: &str = "Data/Prefabs/test/owner.xml";
const DEFAULT: &str = "a0000000-0000-0000-0000-000000000001";

#[test]
fn preserves_prefab_remappings_per_occurrence_and_proves_original_fields() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    write_archive(&path)?;

    let output = p4kx_testkit::p4kx()?
        .args(["--json", "--key", &test_key().to_hex(), "prefab", "dump"])
        .arg(&path)
        .args(["test/root", "root"])
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_mappings(&report)?;
    assert_pair_origin(&report)?;

    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    let result = PrefabSet::new(&archive).resolve("test/root", "root")?;
    let [first, ..] = result.guid_remapping_instances() else {
        return Err("missing SDK remapping occurrence".into());
    };
    let Some(pair) = first.blocks().first().and_then(|block| block.first()) else {
        return Err("missing SDK pair".into());
    };
    assert_eq!(
        first
            .default_objects(pair)
            .map(|location| location.object_index)
            .collect::<Vec<_>>(),
        [0, 1],
        "all matching flattened defaults remain observable"
    );
    Ok(())
}

fn write_archive(path: &Path) -> TestResult {
    let root = concat!(
        "<PrefabsLibrary><Prefab Name='root'><Objects>",
        "<Object Type='Prefab' Name='same' PrefabName='test/owner.middle'/>",
        "<Object Type='Prefab' Name='same' PrefabName='test/owner.middle'/>",
        "</Objects></Prefab></PrefabsLibrary>",
    );
    let owner = concat!(
        "<PrefabsLibrary><Prefab Name='middle'><Objects>",
        "<Object Type='Prefab' Name='inner' PrefabName='test/target.child'>",
        "<GUIDRemapping>",
        "<Object defaultGUID='{A0000000-0000-0000-0000-000000000001}' instanceGUID='first'/>",
        "<Object defaultGUID='bad'/>",
        "<Object defaultGUID='bad' instanceGUID=''/>",
        "</GUIDRemapping><GUIDRemapping/>",
        "</Object>",
        "<Object Type='Prefab' Name='absent' PrefabName='test/target.none'>",
        "<GUIDRemapping><Object instanceGUID='still-authored'/></GUIDRemapping>",
        "</Object></Objects></Prefab></PrefabsLibrary>",
    );
    let target = format!(
        "<PrefabsLibrary><Prefab Name='child'><Objects>\
         <Object Type='Entity' Id='{DEFAULT}' Name='matching'/>\
         <Object Type='Entity' Id='{DEFAULT}' Name='duplicate'/>\
         </Objects></Prefab></PrefabsLibrary>"
    );
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Prefabs/test/root.xml",
            root.as_bytes(),
        ))
        .entry(TestEntry::stored(OWNER, owner.as_bytes()).encrypted())
        .entry(TestEntry::stored(
            "Data/Prefabs/test/target.xml",
            target.as_bytes(),
        ))
        .write_to(path)?;
    Ok(())
}

fn assert_mappings(report: &Value) -> TestResult {
    let mappings = report["guid_remappings"]
        .as_array()
        .ok_or("remapping occurrences")?;
    let [first, missing, repeated, repeated_missing] = mappings.as_slice() else {
        return Err("four distinct remapping occurrences".into());
    };
    for (row, path) in [
        (first, [0, 0]),
        (missing, [0, 1]),
        (repeated, [1, 0]),
        (repeated_missing, [1, 1]),
    ] {
        assert_eq!(
            row.get("instance_path"),
            Some(&json!(path)),
            "unique occurrence route"
        );
    }
    assert_eq!(
        first.get("target"),
        Some(&json!({"file": "test/target", "prefab_index": 0})),
        "typed target uses the entered prefab"
    );
    assert_eq!(
        missing.get("target"),
        Some(&Value::Null),
        "missing target stays explicit"
    );
    assert_eq!(
        first.pointer("/blocks/1"),
        Some(&json!([])),
        "empty block is retained"
    );
    assert_eq!(
        first.pointer("/blocks/0/1"),
        Some(&json!({"default_guid": "bad", "instance_guid": null})),
        "malformed and absent original spellings remain distinct"
    );
    assert_eq!(
        first.pointer("/blocks/0/2/instance_guid"),
        Some(&json!("")),
        "empty value is authored"
    );
    assert_eq!(
        missing.pointer("/blocks/0/0/instance_guid"),
        Some(&json!("still-authored")),
        "unresolved instances retain their authored remaps"
    );
    assert_eq!(
        report.pointer("/objects/0/id"),
        Some(&json!(DEFAULT)),
        "the resolved object keeps its source ID"
    );
    Ok(())
}

fn assert_pair_origin(report: &Value) -> TestResult {
    let origin = report
        .get("fieldOrigins")
        .and_then(|fields| fields.get("/guid_remappings/0/blocks/0/0/default_guid"))
        .ok_or("default GUID origin")?;
    assert_eq!(
        origin["containsEncryptedSource"], true,
        "the stored pair belongs to the encrypted owner"
    );
    assert_eq!(
        origin["sourceSetComplete"], true,
        "the complete pair belongs to a known source"
    );
    let refs = origin["sourceUsesRefs"]
        .as_array()
        .ok_or("field source references")?;
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let raw = uses
        .iter()
        .find(|usage| {
            refs.contains(&usage["id"])
                && usage["role"] == "payload"
                && usage.pointer("/location/field").and_then(Value::as_str) == Some("defaultGUID")
        })
        .ok_or("original defaultGUID attribute source")?;
    let sources = report
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("source inventory")?;
    let owner = sources
        .iter()
        .find(|source| source["id"] == raw["source"])
        .ok_or("physical owner")?;
    assert_eq!(owner["entryIndex"], 1, "original remap Entry index");
    assert_eq!(
        owner["storedEncryption"], "encrypted",
        "physical owner is encrypted"
    );
    Ok(())
}
