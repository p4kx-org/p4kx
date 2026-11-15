// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Per-value sources for Prefab dump rows over a synthetic Archive.

use std::error::Error;

use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::{TestResult, original_bytes, run};

#[path = "prefab_dump_diagnostics.rs"]
mod diagnostics;

const ROOT: &str = concat!(
    "<PrefabsLibrary Name='Root'><Prefab Name='Start'><Objects>",
    "<Object Type='Entity' Name='plain'><PropertiesDataCore/></Object>",
    "<Object Type='Prefab' Name='plain-instance' PrefabName='test/mid.Mid' Pos='2,0,0'/>",
    "<Object Type='Prefab' Name='failed' PrefabName='test/broken.Broken'/>",
    "<Object Type='Prefab' Name='unknown' PrefabName='test/missing.Gone' PrefabGUID='no-match'/>",
    "</Objects></Prefab></PrefabsLibrary>",
);
const MID: &str = concat!(
    "<PrefabsLibrary Name='MidLib'><Prefab Name='Mid'><Objects>",
    "<Object Type='Group' Id='group' Pos='10,0,0' Rotate='1,0,0,0'><Objects>",
    "<Object Type='Prefab' Name='encrypted-selector' PrefabName='test/child.Leaf' Pos='5,0,0'/>",
    "</Objects></Object></Objects></Prefab></PrefabsLibrary>",
);
const CHILD: &str = concat!(
    "<PrefabsLibrary Name='Child'><Prefab Name='Leaf'><Objects>",
    "<Object Type='Brush' Name='nested-leaf' Prefab='Objects/test/unused.cgf' ",
    "Pos='1,0,0' Rotate='0.5,0.5,0.5,0.5' Scale='2,2,2'/>",
    "</Objects></Prefab></PrefabsLibrary>",
);
const BROKEN: &str = "Data/Prefabs/test/broken.xml";
const UNUSED: &str = "Data/Objects/test/unused.cgf";

fn dump() -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Prefabs/test/root.xml",
            ROOT.as_bytes(),
        ))
        .entry(TestEntry::stored("Data/Prefabs/test/mid.xml", MID.as_bytes()).encrypted())
        .entry(TestEntry::stored(
            "Data/Prefabs/test/child.xml",
            CHILD.as_bytes(),
        ))
        .entry(TestEntry::stored(BROKEN, b"<NotALibrary/>").encrypted())
        .entry(TestEntry::stored(UNUSED, b"unused model payload").encrypted())
        .write_to(&path)?;
    let output = run(&["dump", "test/root", "Start"], &path, true)?;
    assert_eq!(output.code, Some(0), "{}", output.stderr);
    Ok(serde_json::from_str(&output.stdout)?)
}

#[test]
fn a_plain_leaf_retains_active_encrypted_selectors_and_exact_transform_roles() -> TestResult {
    let report = dump()?;
    assert_eq!(
        report["prefabs_entered"], 3,
        "root and both visited Prefabs"
    );
    assert_eq!(
        report.pointer("/objects/0/name").and_then(Value::as_str),
        Some("plain")
    );
    assert_eq!(
        report.pointer("/objects/1/name").and_then(Value::as_str),
        Some("nested-leaf")
    );
    assert_eq!(
        report
            .pointer("/objects/1/position/0")
            .and_then(Value::as_f64),
        Some(18.0)
    );
    assert_eq!(field(&report, "/file")?["containsEncryptedSource"], false);
    assert_eq!(
        field(&report, "/objects/0/name")?["containsEncryptedSource"],
        false
    );
    let nested = field(&report, "/objects/1/name")?;
    assert_eq!(nested["containsEncryptedSource"], true);
    assert_eq!(nested["sourceSetComplete"], true);
    let uses = original_uses(&report, nested)?;
    assert!(
        uses.iter().any(|usage| {
            usage["role"] == "selector"
                && source(&report, usage)
                    .is_ok_and(|physical| physical["kind"] == "archiveDirectory")
        }),
        "the accepted name route retains its library-prefix directory survey"
    );
    exact_site(
        &report,
        &uses,
        MID,
        "mid.xml",
        "PrefabName='test/child.Leaf'",
        "selector",
    );
    exact_site(
        &report,
        &uses,
        CHILD,
        "child.xml",
        "Name='nested-leaf'",
        "payload",
    );
    assert_transform_uses(&report)?;
    assert_resource_absence_and_matrix_cells(&report)?;
    Ok(())
}

fn assert_transform_uses(report: &Value) -> TestResult {
    let translation = original_uses(report, field(report, "/objects/1/position")?)?;
    exact_site(
        report,
        &translation,
        MID,
        "mid.xml",
        "Rotate='1,0,0,0'",
        "conversion",
    );
    exact_site(
        report,
        &translation,
        CHILD,
        "child.xml",
        "Pos='1,0,0'",
        "conversion",
    );
    exact_site(
        report,
        &translation,
        CHILD,
        "child.xml",
        "Rotate='0.5,0.5,0.5,0.5'",
        "selector",
    );
    let matrix = original_uses(report, field(report, "/objects/1/transform")?)?;
    exact_site(
        report,
        &matrix,
        CHILD,
        "child.xml",
        "Rotate='0.5,0.5,0.5,0.5'",
        "conversion",
    );
    assert!(
        !translation.iter().any(|usage| {
            usage["role"] == "conversion"
                && entry_path(report, usage).is_ok_and(|path| path.ends_with("child.xml"))
                && original_bytes(CHILD, usage).is_ok_and(|bytes| bytes.starts_with("Rotate="))
        }),
        "leaf rotation contributes numerically to the matrix rather than its translation"
    );
    Ok(())
}

fn assert_resource_absence_and_matrix_cells(report: &Value) -> TestResult {
    let model = field(report, "/objects/0/model")?;
    assert!(
        original_uses(report, model)?.iter().any(|usage| {
            usage["role"] == "selector"
                && usage.pointer("/location/field").and_then(Value::as_str)
                    == Some("EntityGeometryResource")
                && original_bytes(ROOT, usage).is_ok_and(|xml| xml == "<PropertiesDataCore/>")
        }),
        "the missing child points to its actual containing XML node"
    );
    let position = field(report, "/objects/1/position/0")?;
    assert_eq!(
        position,
        field(report, "/objects/1/transform/0/3")?,
        "the translation is the last column of the affine transform"
    );
    let linear = original_uses(report, field(report, "/objects/1/transform/0/0")?)?;
    exact_site(
        report,
        &linear,
        CHILD,
        "child.xml",
        "Pos='1,0,0'",
        "selector",
    );
    exact_site(
        report,
        &linear,
        CHILD,
        "child.xml",
        "Rotate='0.5,0.5,0.5,0.5'",
        "conversion",
    );
    Ok(())
}

#[test]
fn unsuccessful_reads_do_not_mark_the_emitted_value_and_guid_absence_stays_incomplete() -> TestResult
{
    let report = dump()?;
    let failed = field(&report, "/objects/2/unexpanded")?;
    assert_eq!(failed["containsEncryptedSource"], false);
    assert_eq!(failed["sourceSetComplete"], true);
    assert!(
        original_uses(&report, failed)?.iter().all(|usage| {
            !entry_path(&report, usage).is_ok_and(|path| path.ends_with("broken.xml"))
        }),
        "the failed target bytes never contributed"
    );
    let unknown = field(&report, "/objects/3/unexpanded")?;
    assert_eq!(unknown["sourceSetComplete"], false);
    assert!(
        unknown["containsEncryptedSource"].is_null(),
        "GUID absence evidence is incomplete"
    );
    let inventory = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("uses")?;
    let attempt = inventory
        .iter()
        .find(|usage| {
            usage["role"] == "attempted"
                && entry_path(&report, usage).is_ok_and(|path| path.ends_with("broken.xml"))
        })
        .ok_or("failed encrypted target attempt")?;
    assert_eq!(source(&report, attempt)?["storedEncryption"], "encrypted");
    diagnostics::assert_text_root(&report)?;
    assert!(
        inventory.iter().all(|usage| {
            !entry_path(&report, usage).is_ok_and(|path| path.ends_with("unused.cgf"))
        }),
        "the named model payload was never opened"
    );
    assert_all_dump_paths_have_origins(&report)
}

fn assert_all_dump_paths_have_origins(report: &Value) -> TestResult {
    for path in [
        "/archive",
        "/library",
        "/name",
        "/file",
        "/prefabs_entered",
        "/objects",
        "/issues",
    ] {
        let _ = field(report, path)?;
    }
    for collection in ["objects", "issues"] {
        let rows = report
            .get(collection)
            .and_then(Value::as_array)
            .ok_or("collection rows")?;
        for (index, row) in rows.iter().enumerate() {
            let path = format!("/{collection}/{index}");
            let _ = field(report, &path)?;
            for key in row.as_object().ok_or("row fields")?.keys() {
                let _ = field(report, &format!("{path}/{key}"))?;
            }
        }
    }
    Ok(())
}

fn field<'a>(report: &'a Value, path: &str) -> Result<&'a Value, Box<dyn Error>> {
    report
        .get("fieldOrigins")
        .and_then(|origins| origins.get(path))
        .ok_or_else(|| format!("missing origin {path}").into())
}

fn original_uses<'a>(report: &'a Value, origin: &Value) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("uses")?;
    let ids = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source use refs")?;
    ids.iter()
        .map(|id| {
            uses.iter()
                .find(|usage| &usage["id"] == id)
                .ok_or_else(|| "unresolved producer source use".into())
        })
        .collect()
}

fn source<'a>(report: &'a Value, usage: &Value) -> Result<&'a Value, Box<dyn Error>> {
    report
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("sources")?
        .iter()
        .find(|source| source["id"] == usage["source"])
        .ok_or_else(|| "unresolved physical source".into())
}

fn entry_path<'a>(report: &'a Value, usage: &Value) -> Result<&'a str, Box<dyn Error>> {
    source(report, usage)?
        .pointer("/path/value")
        .and_then(Value::as_str)
        .ok_or_else(|| "Entry path".into())
}

fn exact_site(
    report: &Value,
    uses: &[&Value],
    xml: &str,
    filename: &str,
    literal: &str,
    role: &str,
) {
    assert!(
        uses.iter().any(|usage| {
            usage["role"] == role
                && entry_path(report, usage).is_ok_and(|path| path.ends_with(filename))
                && original_bytes(xml, usage).is_ok_and(|bytes| bytes == literal)
        }),
        "missing original {role} {filename} {literal}"
    );
}
