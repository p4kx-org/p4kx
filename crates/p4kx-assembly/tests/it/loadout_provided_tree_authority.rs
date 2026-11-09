// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_archive::{Archive, SourceEncryption};
use p4kx_assembly::{Entities, LoadoutReadEvent, LoadoutReadObserver, LoadoutResolver};
use p4kx_dataforge::{DataForge, Record, RecordTree, Value};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const GUID: u128 = 0x8a00_0000_0000_4000_8000_0000_0000_0001;
const LEFT: &str = "synthetic/left.xml";
const RIGHT: &str = "synthetic/right.xml";

#[derive(Default)]
struct Probe {
    provided: AtomicUsize,
    canonical: AtomicUsize,
}

impl LoadoutReadObserver for Probe {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        match event {
            LoadoutReadEvent::ProvidedTree { .. } => {
                self.provided.fetch_add(1, Ordering::Relaxed);
            }
            LoadoutReadEvent::CanonicalRead { outcome, .. } => {
                assert!(
                    outcome.is_ok(),
                    "the actual synthetic source decodes successfully"
                );
                self.canonical.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}

fn database(path: &str) -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(GUID, "SameHeader", "synthetic/entity.xml")
                .loadout(TestLoadout::Xml(path.to_owned()))
                .record(),
        )
        .build()
}

fn archive(
    directory: &Path,
    name: &str,
    path: &str,
    encrypted: bool,
) -> Result<Archive, Box<dyn std::error::Error>> {
    let destination = directory.join(name);
    let entry = TestEntry::stored("Data/Game2.dcb", database(path));
    let entry = if encrypted { entry.encrypted() } else { entry };
    ArchiveBuilder::new().entry(entry).write_to(&destination)?;
    Ok(Archive::open_with_key(
        destination,
        Some(p4kx_testkit::test_key()),
    )?)
}

fn original_bytes(
    archive: &Archive,
    encrypted: bool,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Read as _;

    assert_eq!(
        archive.entries().len(),
        1,
        "one actual original DataForge input is admitted"
    );
    let entry = archive
        .entries()
        .first()
        .ok_or("original DataForge Entry missing")?;
    assert_eq!(
        entry.path(),
        "Data/Game2.dcb",
        "the actual Entry is the declared synthetic database"
    );
    let origin = archive
        .entry_origin(entry)
        .ok_or("original admitted Entry evidence missing")?;
    let expected = if encrypted {
        SourceEncryption::Encrypted
    } else {
        SourceEncryption::Unencrypted
    };
    assert_eq!(
        origin.encryption(),
        expected,
        "the two original archives have different genuine encryption evidence"
    );
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn original_path<'source>(
    tree: &RecordTree<'source>,
) -> Result<&'source str, Box<dyn std::error::Error>> {
    let Some(Value::Array(components)) = tree.field("Components") else {
        return Err("actual Components array missing".into());
    };
    for component in components {
        let Value::Struct(component) = component else {
            continue;
        };
        if component.type_name() != "SEntityComponentDefaultLoadoutParams" {
            continue;
        }
        let Some(Value::Struct(loadout)) = component.field("loadout") else {
            return Err("actual owned XML loadout missing".into());
        };
        let Some(Value::String(path)) = loadout.field("loadoutPath") else {
            return Err("actual authored XML path missing".into());
        };
        return Ok(*path);
    }
    Err("actual default-loadout component missing".into())
}

fn assert_same_owner<'source>(
    forge: &'source DataForge<'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    expected: &str,
) -> TestResult {
    let observer = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(forge)), observer.clone());
    let forwarded = tree.clone();
    let node = resolver.resolve_tree(record, &forwarded)?;
    assert_eq!(
        node.loadout_file(),
        Some(expected),
        "a genuine unchanged clone preserves its actual original value"
    );
    assert_eq!(
        observer.provided.load(Ordering::Relaxed),
        1,
        "a genuine supplied tree is observed once"
    );
    assert_eq!(
        observer.canonical.load(Ordering::Relaxed),
        0,
        "a valid same-owner tree is never decoded again"
    );
    Ok(())
}

fn assert_original_trees(
    record: &Record<'_>,
    foreign: &Record<'_>,
    own_tree: &RecordTree<'_>,
    foreign_tree: &RecordTree<'_>,
) -> TestResult {
    assert_eq!(
        record.guid(),
        foreign.guid(),
        "GUID equality must not make source owners interchangeable"
    );
    assert_eq!(
        record.index(),
        foreign.index(),
        "the physical numeric index is deliberately identical"
    );
    assert_eq!(
        own_tree.error_count(),
        0,
        "the actual source tree is fully readable"
    );
    assert_eq!(
        foreign_tree.error_count(),
        0,
        "the foreign source tree is fully readable"
    );
    assert_eq!(
        (own_tree.guid(), own_tree.record_index()),
        (foreign_tree.guid(), foreign_tree.record_index()),
        "every descriptor checked by the old admission rule is equal"
    );
    assert_eq!(
        original_path(own_tree)?,
        LEFT,
        "the actual original getter proves the left path before resolution"
    );
    assert_eq!(
        original_path(foreign_tree)?,
        RIGHT,
        "the actual foreign getter proves the distinct right path"
    );
    Ok(())
}

#[test]
fn a_foreign_same_guid_index_tree_cannot_replace_the_actual_records_original_values() -> TestResult
{
    let directory = tempfile::tempdir()?;
    let left_archive = archive(directory.path(), "plain.p4k", LEFT, false)?;
    let right_archive = archive(directory.path(), "encrypted.p4k", RIGHT, true)?;
    let left_bytes = original_bytes(&left_archive, false)?;
    let right_bytes = original_bytes(&right_archive, true)?;
    let left = DataForge::open(&left_bytes)?;
    let right = DataForge::open(&right_bytes)?;
    let record = left.record(0).ok_or("actual left Record missing")?;
    let foreign = right.record(0).ok_or("actual foreign Record missing")?;
    assert!(
        record.is_from(&left) && !record.is_from(&right),
        "the actual Record retains its borrowed source owner"
    );
    let own_tree = record.tree()?;
    let foreign_tree = foreign.tree()?;
    assert_original_trees(&record, &foreign, &own_tree, &foreign_tree)?;
    assert_same_owner(&left, &record, &own_tree, LEFT)?;
    assert_same_owner(&right, &foreign, &foreign_tree, RIGHT)?;
    let observer = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&left)), observer.clone());
    let node = resolver.resolve_tree(&record, &foreign_tree)?;
    assert_eq!(
        node.loadout_file(),
        Some(LEFT),
        "a foreign tree cannot lend its values to this actual Record"
    );
    assert_eq!(
        observer.provided.load(Ordering::Relaxed),
        0,
        "a foreign tree is not certified as this Record's provided input"
    );
    assert_eq!(
        observer.canonical.load(Ordering::Relaxed),
        1,
        "the actual Record is decoded once after rejecting the foreign tree"
    );
    Ok(())
}
