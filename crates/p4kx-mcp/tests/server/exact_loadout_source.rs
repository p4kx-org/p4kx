// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveFiles, Attachment, EntityNode, LoadoutResolver, Target};
use p4kx_extraction::{Archive, DataForgeEntry};

use crate::exact_loadout_fixture::{self as fixture, Texts};
use crate::support::{Install, TestResult};

pub(crate) fn original(install: &Install, texts: &Texts) -> TestResult<EntityNode> {
    let archive = Archive::open(install.archive())?;
    let loaded = DataForgeEntry::read(&archive)?;
    let forge = loaded.open()?;
    let record = forge.record(0).ok_or("actual root Record")?;
    assert_eq!(
        record.name()?,
        format!("EntityClassDefinition.{}", texts.class),
        "the actual Record name carries the entire original class"
    );
    let original_path = texts.file.strip_suffix(".xml").ok_or("fixture extension")?;
    assert_eq!(
        record.record_path()?,
        original_path,
        "original normalized path"
    );
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the tree belongs to the actual input Record"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "all original field declarations decode"
    );
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let selected = resolver.entities().find(&texts.class)?;
    assert_eq!(
        selected.index(),
        0,
        "full class lookup avoids the same-prefix decoy"
    );
    let node = resolver.resolve(&selected)?;
    assert_eq!(
        node.class_name(),
        texts.class,
        "original resolved root class"
    );
    assert_eq!(node.name(), record.name()?, "original resolved Record name");
    assert_eq!(
        node.record_path(),
        original_path,
        "original resolved Record path"
    );
    check_ports(&node, texts)?;
    Ok(node)
}

fn check_ports(node: &EntityNode, texts: &Texts) -> TestResult {
    assert_eq!(node.ports().len(), 2, "two original ordered ports");
    let port = node.ports().first().ok_or("original attached port")?;
    assert_eq!(port.name(), texts.port, "original complete port name");
    let first = node
        .walk()
        .into_iter()
        .next()
        .ok_or("original walk path")?
        .0;
    assert_eq!(
        first, texts.port,
        "the actual full port path exceeds the old path clip"
    );
    let Attachment::Entity(child) = port.attachment() else {
        return Err("the first original port must resolve to the child".into());
    };
    assert_eq!(
        child.class_name(),
        texts.child,
        "original attached child identity"
    );
    let definition = port.definition().ok_or("original port definition")?;
    assert_eq!(
        definition.display_name(),
        texts.display,
        "original nullable Locale value"
    );
    assert_eq!(
        definition.flags(),
        texts.flags,
        "original flags including edge whitespace"
    );
    assert_eq!(
        definition
            .transform()
            .ok_or("original bone helper")?
            .helper(),
        texts.helper,
        "original helper value"
    );
    assert_eq!(
        definition.types().len(),
        texts.types.len(),
        "all original types were read"
    );
    for (actual, (name, subtypes)) in definition.types().iter().zip(&texts.types) {
        assert_eq!(actual.name(), name, "each original type spelling");
        assert_eq!(
            actual.sub_types(),
            subtypes,
            "all original subtype spellings"
        );
    }
    check_unresolved(node, texts)
}

fn check_unresolved(node: &EntityNode, texts: &Texts) -> TestResult {
    let missing = node.ports().get(1).ok_or("original unresolved port")?;
    assert_eq!(
        missing.name(),
        fixture::MISSING_PORT,
        "original second port"
    );
    let Attachment::Unresolved(failed) = missing.attachment() else {
        return Err("the original missing class must remain unresolved".into());
    };
    let Target::ClassName(name) = failed.target() else {
        return Err("the original target must be a class name".into());
    };
    assert_eq!(name, &texts.missing, "entire original unresolved target");
    assert_eq!(
        failed.error().kind(),
        "unknown_class",
        "actual original failure kind"
    );
    assert_eq!(
        failed.error().to_string(),
        format!("no Entity has the class name {}", texts.missing),
        "actual full original Display reason before MCP"
    );
    Ok(())
}
