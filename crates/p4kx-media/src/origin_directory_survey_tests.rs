// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, SourceUses};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::ReadInventory;
use crate::Artifact;
use crate::checkpoint::Checkpoint;
use crate::origin_contract::{OriginProtocol, OriginReceipts};
use crate::origin_index_test_support;
use crate::origin_read_policy::OriginReadPolicy;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn directory_survey_accepts_an_observed_empty_root() -> TestResult {
    origin_index_test_support::run(|root, checkpoint| {
        let (source, usage) = survey(root)?;
        check(root, checkpoint, &source, &usage)
    })
}

#[test]
fn directory_survey_refuses_unproved_ranges_roles_and_domains() -> TestResult {
    origin_index_test_support::run(|root, checkpoint| {
        let (source, usage) = survey(root)?;
        for (pointer, value) in [
            ("/location/startEntryIndex", json!("1")),
            ("/location/endEntryIndex", json!("1")),
            ("/role", json!("payload")),
            ("/scope", json!("payload")),
            ("/effectiveEncryption", json!("unknown")),
        ] {
            let mut forged = usage.clone();
            *forged.pointer_mut(pointer).ok_or("original use field")? = value;
            assert!(
                check(root, checkpoint, &source, &forged).is_err(),
                "{pointer}"
            );
        }
        let mut forged = source;
        *forged
            .pointer_mut("/admittedEntryCount")
            .ok_or("original count")? = json!("1");
        assert!(check(root, checkpoint, &forged, &usage).is_err());
        Ok(())
    })
}

fn survey(root: &Path) -> TestResult<(Value, Value)> {
    let archive = Archive::open(root.join("Data.p4k"))?;
    let sources = SourceUses::new();
    let usage = sources
        .observe_root_directory_paths(&archive)?
        .ok_or("original parsed directory")?;
    let inventory = sources.freeze()?;
    let source = inventory.sources().first().ok_or("directory source")?;
    Ok((
        serde_json::to_value(source)?,
        serde_json::to_value(usage.description())?,
    ))
}

fn check(root: &Path, checkpoint: Checkpoint<'_>, source: &Value, usage: &Value) -> TestResult {
    let receipts = OriginReceipts {
        protocol: OriginProtocol::V1,
        sources: write_row(root, "origin-sources.jsonl", source)?,
        uses: write_row(root, "origin-uses.jsonl", usage)?,
        fields: origin_index_test_support::artifact("field-origins.jsonl"),
        streams: Vec::new(),
    };
    let policy = OriginReadPolicy::from_receipts(&receipts);
    ReadInventory::open(root, &receipts, checkpoint, policy)?;
    Ok(())
}

fn write_row(root: &Path, name: &str, row: &Value) -> TestResult<Artifact> {
    let mut bytes = serde_json::to_vec(row)?;
    bytes.push(b'\n');
    fs::write(root.join(name), &bytes)?;
    let mut artifact = origin_index_test_support::artifact(name);
    artifact.size = bytes.len() as u64;
    artifact.digest.hex = crate::hashing::hex(Sha256::digest(&bytes).into());
    Ok(artifact)
}

#[test]
fn a_path_order_subtree_is_bounded_by_the_admitted_root_directory() -> TestResult {
    use p4kx_extraction::EntryTree;
    use p4kx_testkit::{ArchiveBuilder, TestEntry};

    origin_index_test_support::run(|root, checkpoint| {
        let path = root.join("tree.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored("Folder/b.txt", b"b"))
            .entry(TestEntry::stored("a.txt", b"a"))
            .entry(TestEntry::stored("Folder/c.txt", b"c"))
            .write_to(&path)?;
        let archive = Archive::open(path)?;
        let tree = EntryTree::build(&archive);
        let folder = tree.find_folder("Folder").ok_or("fixture folder")?;
        let range = tree.folder(folder).entries();
        assert_eq!(range, 1..3);
        let sources = SourceUses::new();
        let used = sources
            .observe_root_directory_tree_paths(&archive, range)?
            .ok_or("original folder range")?;
        let inventory = sources.freeze()?;
        let source = inventory.sources().first().ok_or("directory source")?;
        let source = serde_json::to_value(source)?;
        let usage = serde_json::to_value(used.description())?;
        check(root, checkpoint, &source, &usage)?;
        for (pointer, value) in [
            ("/location/startEntryIndex", json!("4")),
            ("/location/endEntryIndex", json!("4")),
            ("/location/type", json!("archiveDirectoryPaths")),
        ] {
            let mut forged = usage.clone();
            *forged.pointer_mut(pointer).ok_or("original use field")? = value;
            assert!(
                check(root, checkpoint, &source, &forged).is_err(),
                "{pointer}"
            );
        }
        assert!(
            sources
                .observe_root_directory_tree_paths(&archive, 1..4)
                .is_err()
        );
        Ok(())
    })
}
