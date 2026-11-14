// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveFiles, EntityNode, LoadoutError, LoadoutResolver};
use p4kx_extraction::{Archive, DataForge, DataForgeEntry, Guid, Value};

use crate::complete_loadout_notes_fixture as fixture;
use crate::support::{Install, TestResult};

pub(crate) fn original(install: &Install, count: usize, huge_tail: bool) -> TestResult<EntityNode> {
    let archive = Archive::open(install.archive())?;
    let loaded = DataForgeEntry::read(&archive)?;
    let forge = loaded.open()?;
    assert_eq!(
        forge.records().len(),
        count + 2,
        "root, distinct assortments and actual Tag"
    );
    let root = forge.record(0).ok_or("original root")?;
    assert_eq!(
        root.name()?,
        format!("EntityClassDefinition.{}", fixture::CLASS),
        "original class"
    );
    assert_eq!(
        root.tree()?.error_count(),
        0,
        "all actual assortment References decode"
    );
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let entity = resolver.entities().find(fixture::CLASS)?;
    assert_eq!(
        entity.index(),
        root.index(),
        "actual original selected root"
    );
    let node = resolver.resolve(&entity)?;
    assert!(
        node.issues().is_empty(),
        "undecidable requirements are notes, not errors"
    );
    assert!(
        node.ports().is_empty(),
        "empty original alternatives add no ports"
    );
    assert_eq!(
        node.notes().len(),
        count,
        "all distinct original note occurrences"
    );
    for (ordinal, note) in node.notes().iter().enumerate() {
        let label = fixture::group(ordinal, huge_tail, count);
        let guid = check_record(&forge, ordinal, &label)?;
        check_note(note, guid, &label)?;
    }
    Ok(node)
}

fn check_record(forge: &DataForge<'_>, ordinal: usize, label: &str) -> TestResult<Guid> {
    let position = u32::try_from(ordinal + 1)?;
    let record = forge
        .record(position)
        .ok_or("original physical assortment")?;
    assert_eq!(record.index(), position, "original physical order");
    assert_eq!(
        record.type_name()?,
        "SLoadoutAssortment",
        "actual declared owner type"
    );
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "actual original tree owner");
    assert_eq!(
        tree.error_count(),
        0,
        "all actual requirement values decode"
    );
    let Some(Value::Array(groups)) = tree.field("GroupedLoadouts") else {
        return Err("original declared group array".into());
    };
    assert_eq!(groups.len(), 1, "one authored group");
    let Some(Value::Struct(group)) = groups.first() else {
        return Err("original group instance".into());
    };
    assert!(
        matches!(group.field("Group"), Some(Value::String(actual)) if *actual == label),
        "the exact original group name exists before MCP"
    );
    Ok(record.guid())
}

fn check_note(note: &LoadoutError, guid: Guid, label: &str) -> TestResult {
    let LoadoutError::AssortmentRequirement {
        guid: actual,
        assortment,
        groups,
    } = note
    else {
        return Err("actual original note kind".into());
    };
    assert_eq!(
        *actual, guid,
        "note is paired with its physical original assortment"
    );
    assert!(
        !assortment,
        "only the actual group requirement is undecidable"
    );
    assert_eq!(groups.len(), 1, "one original undecidable group");
    assert_eq!(
        groups.first().map(String::as_str),
        Some(label),
        "exact original group"
    );
    Ok(())
}
