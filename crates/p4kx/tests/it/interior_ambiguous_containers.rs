// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use p4kx_testkit::{TestContainer, TestEntry, TestObjectContainer};
use serde_json::{Value, json};

use crate::entity_package;

type Check<T = ()> = Result<T, Box<dyn Error>>;

const ROOT: &str = "Data/ObjectContainers/root.socpak";
const ROOT_DUPLICATE: &str = "fallback.socpak";
const NESTED_DUPLICATE: &str = "nearest.socpak";
const ROTATION: [f64; 4] = [1.0, 0.0, 0.0, 0.0];

#[test]
fn explicit_ambiguous_container_chain_is_rejected_by_source_and_package() -> Check {
    let dir = tempfile::tempdir()?;
    let archive = archive_fixture(dir.path())?;
    let source = dir.path().join("source.json");
    let package = dir.path().join("package");
    fs::write(&source, "old source")?;

    let cases: [(&[&str], &str, &str); 2] = [
        (
            &[ROOT_DUPLICATE],
            ROOT_DUPLICATE,
            "Archive layer 0 (Entry indexes [1, 2])",
        ),
        (
            &[ROOT, NESTED_DUPLICATE],
            NESTED_DUPLICATE,
            "Archive layer 1 (Entry indexes [0, 1])",
        ),
    ];
    for (chain, selector, candidates) in cases {
        for (command, out) in [("source", &source), ("package", &package)] {
            let result = run(command, &archive, chain, out, &[])?;
            assert_eq!(result.status.code(), Some(2), "{chain:?}: {result:?}");
            let stderr = String::from_utf8(result.stderr)?;
            assert!(stderr.contains("ambiguous"), "{stderr}");
            assert!(stderr.contains(selector), "{stderr}");
            assert!(stderr.contains(candidates), "{stderr}");
        }
        assert_eq!(fs::read_to_string(&source)?, "old source");
        assert!(!package.exists());
    }
    Ok(())
}

#[test]
fn geometry_skips_the_same_ambiguous_children_as_authored_soc() -> Check {
    let dir = tempfile::tempdir()?;
    let archive = archive_fixture(dir.path())?;
    let package = dir.path().join("package");
    let result = run("package", &archive, &[ROOT], &package, &["--authored-soc"])?;
    assert!(result.status.success(), "{result:?}");

    let manifest = entity_package::manifest(&package)?;
    let companion = manifest
        .pointer("/authored_soc/file")
        .and_then(Value::as_str)
        .ok_or("authored companion reference")?;
    let source: Value = serde_json::from_slice(&fs::read(package.join(companion))?)?;
    assert_candidates(&source, "/occurrences/0/children/0/target", [0, 1])?;
    assert_candidates(&source, "/occurrences/0/children/1/target", [1, 2])?;
    assert_eq!(
        source.pointer("/occurrences/0/children/3/target"),
        Some(&json!({"state": "opened", "occurrence": 2}))
    );
    assert_candidates(&source, "/occurrences/2/children/0/target", [0, 1])?;

    let report: Value = serde_json::from_slice(&result.stdout)?;
    assert_eq!(report["containers_opened"], 3);
    let issues = report["issues"].as_array().ok_or("package issues")?;
    for (selector, candidates) in [
        ("nearest", "Archive layer 1 (Entry indexes [0, 1])"),
        ("fallback", "Archive layer 0 (Entry indexes [1, 2])"),
        ("deeper", "Archive layer 2 (Entry indexes [0, 1])"),
    ] {
        assert!(
            issues.iter().any(|issue| issue["name"] == selector
                && issue["kind"] == "container_ambiguous"
                && issue["skipped"] == true
                && issue["reason"]
                    .as_str()
                    .is_some_and(|reason| reason.contains(candidates))),
            "{selector}: {issues:?}"
        );
    }
    Ok(())
}

fn assert_candidates(scene: &Value, target: &str, entry_indexes: [u64; 2]) -> Check {
    let target = scene.pointer(target).ok_or("authored child target")?;
    assert_eq!(
        target["state"], "ambiguous",
        "an exact match has duplicates"
    );
    let candidates = target["candidates"]
        .as_array()
        .ok_or("ambiguity candidates")?;
    let [first, second] = candidates.as_slice() else {
        return Err("expected both physical candidates".into());
    };
    assert_ne!(
        first, second,
        "physical candidates have distinct Entry indexes"
    );
    let sources = scene
        .pointer("/sourceInventory/sources")
        .and_then(Value::as_array)
        .ok_or("authored source inventory")?;
    for (candidate, expected) in [first, second].into_iter().zip(entry_indexes) {
        let physical = sources
            .iter()
            .find(|source| &source["id"] == candidate)
            .ok_or("candidate source reference")?;
        assert_eq!(physical["entryIndex"], expected, "{physical}");
    }
    Ok(())
}

fn archive_fixture(dir: &Path) -> Check<PathBuf> {
    let descriptor = TestObjectContainer::new("root")
        .child("nearest", [1.0, 0.0, 0.0], ROTATION)
        .child("fallback", [2.0, 0.0, 0.0], ROTATION)
        .child("earlier", [3.0, 0.0, 0.0], ROTATION)
        .child("local", [4.0, 0.0, 0.0], ROTATION);
    let local_descriptor =
        TestObjectContainer::new("local").child("deeper", [1.0, 0.0, 0.0], ROTATION);
    let local = TestContainer::new()
        .entry(TestEntry::stored("deeper.socpak", leaf("deeper")))
        .entry(TestEntry::stored("DEEPER.SOCPAK", leaf("deeper")))
        .entry(TestEntry::stored("local.xml", local_descriptor.xml()));
    let root = TestContainer::new()
        .entry(TestEntry::stored(NESTED_DUPLICATE, leaf("nearest")))
        .entry(TestEntry::stored("NEAREST.SOCPAK", leaf("nearest")))
        .entry(TestEntry::stored("earlier.socpak", leaf("earlier")))
        .entry(TestEntry::stored("local.socpak", local.build()))
        .entry(TestEntry::stored("root.xml", descriptor.xml()));
    entity_package::write_archive(
        dir,
        vec![
            TestEntry::stored(ROOT, root.build()),
            TestEntry::stored(ROOT_DUPLICATE, leaf("fallback")),
            TestEntry::stored("FALLBACK.SOCPAK", leaf("fallback")),
            TestEntry::stored("Data/fallback.socpak", leaf("fallback")),
            TestEntry::stored("Data/nearest.socpak", leaf("nearest")),
            TestEntry::stored("local.socpak", leaf("local")),
            TestEntry::stored("LOCAL.SOCPAK", leaf("local")),
        ],
    )
}

fn leaf(name: &str) -> Vec<u8> {
    TestContainer::new()
        .entry(TestEntry::stored(
            format!("{name}.xml"),
            TestObjectContainer::new(name).xml(),
        ))
        .build()
}

fn run(
    subcommand: &str,
    archive: &Path,
    chain: &[&str],
    out: &Path,
    extra: &[&str],
) -> Check<Output> {
    Ok(p4kx_testkit::p4kx()?
        .args(["--json", "interior", subcommand])
        .arg(archive)
        .args(chain)
        .arg("--out")
        .arg(out)
        .args(extra)
        .output()?)
}
