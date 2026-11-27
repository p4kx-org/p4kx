// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{Entities, LoadoutError, LoadoutFiles, LoadoutResolver};
use p4kx_extraction::{DataForge, Guid, SourceLocation, Value};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestLoadout, TestLoadoutEntry};

use crate::Kind;
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_read::original::{ExampleBodyReceipt, OriginalFlowFields, OriginalValuePart};
use crate::contract_loadout::EntityKind;
use crate::export_ledger::ExportLedger;
use crate::loadout::ledger::Capture;

pub(super) const XML_PATH: &str = "synthetic/class-slots.xml";

pub(super) fn database(xml: bool) -> Vec<u8> {
    let loadout = if xml {
        TestLoadout::Xml(XML_PATH.to_owned())
    } else {
        TestLoadout::Manual(vec![
            TestLoadoutEntry::reference("first", 2),
            TestLoadoutEntry::reference("second", 3),
        ])
    };
    let mut builder = p4kx_testkit::entity_schema(DataForgeBuilder::new()).record(
        TestEntity::new(1, "Self", "synthetic/first.xml")
            .loadout(loadout)
            .record(),
    );
    for (guid, name) in [(2, "First"), (3, "Second")] {
        builder = builder.record(
            TestEntity::new(guid, name, format!("synthetic/{name}.xml"))
                .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
                    "self", "Self",
                )]))
                .record(),
        );
    }
    builder
        .record(TestEntity::new(1, "Self", "synthetic/duplicate.xml").record())
        .build()
}

pub(super) fn original(forge: &DataForge<'_>) -> TestResult<Guid> {
    assert_eq!(
        forge.records().count(),
        4,
        "the four actual encoded definitions precede production"
    );
    let first = forge.record(0).ok_or("original first definition")?;
    let duplicate = forge.record(3).ok_or("original later duplicate")?;
    assert_eq!(
        first.guid(),
        duplicate.guid(),
        "the original nominal target GUIDs coincide"
    );
    assert_eq!(
        first.name()?,
        "EntityClassDefinition.Self",
        "the actual first class name"
    );
    assert_eq!(
        first.name()?,
        duplicate.name()?,
        "the original class names intentionally coincide"
    );
    assert_ne!(
        first.record_path()?,
        duplicate.record_path()?,
        "the physical definitions have distinct paths"
    );
    assert_ne!(
        first.index(),
        duplicate.index(),
        "definition order is an original physical distinction"
    );
    for record in forge.records() {
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record),
            "the original actual tree belongs to its own Record"
        );
        assert_eq!(
            tree.error_count(),
            0,
            "all original input values are readable"
        );
    }
    Ok(first.guid())
}

pub(super) fn original_class_locations(
    forge: &DataForge<'_>,
    input: &OriginalCatalogInput<'_, '_>,
) -> TestResult<Vec<SourceLocation>> {
    let mut locations = Vec::new();
    for index in [1, 2] {
        let record = forge.record(index).ok_or("original child referrer")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let fact = fields
            .facts()
            .iter()
            .find(|fact| fact.field().name() == "entityClassName")
            .ok_or("original selected class slot")?;
        let parent = fields.guard_parent(fact)?;
        assert_eq!(
            parent.type_name(),
            "SItemPortLoadoutEntryParams",
            "the independently decoded actual parent"
        );
        assert_eq!(
            fact.value(),
            &Value::String("Self"),
            "the encoded class fallback is an actual String"
        );
        assert_eq!(
            parent.field("entityClassReference"),
            Some(&Value::Null),
            "the Reference does not override this fallback"
        );
        locations.push(fact.location(OriginalValuePart::Value));
    }
    assert_ne!(
        locations.first(),
        locations.get(1),
        "equal class text retains different source referrers"
    );
    Ok(locations)
}

pub(super) fn body<'input>(
    forge: &'input DataForge<'input>,
    input: &'input OriginalCatalogInput<'input, 'input>,
    files: Option<&'input dyn LoadoutFiles>,
    expected: Guid,
    count: usize,
) -> TestResult<ExampleBodyReceipt> {
    let record = forge.record(0).ok_or("the actual original root")?;
    let capture = Arc::new(Capture::for_input(
        input,
        &ExportLedger::default(),
        None,
        Kind::Loadouts,
    ));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(forge)), capture.clone());
    if let Some(files) = files {
        resolver = resolver.with_files(files);
    }
    let node = resolver.resolve(&record)?;
    let again = resolver.resolve(&record)?;
    assert_eq!(
        node, again,
        "cached actual class selections preserve semantic resolution"
    );
    let unresolved: Vec<_> = node.unresolved().collect();
    assert_eq!(
        unresolved.len(),
        count,
        "every actual class cycle remains present"
    );
    assert!(
        unresolved
            .iter()
            .all(|(_, unresolved)| matches!(unresolved.error(),
        LoadoutError::Cycle { guid } if guid.guid() == expected && guid.class_selection().is_some()
            && guid.selection().is_none())),
        "each typed failure keeps the actual distinct class origin"
    );
    let before = input.origins().uses.freeze()?;
    capture.root(&record, EntityKind::Ship, Ok(&node));
    let ledger = capture.finish()?;
    assert_eq!(
        before,
        input.origins().uses.freeze()?,
        "unretained diagnostic factories add no payload"
    );
    let diagnostic = ledger
        .field_diagnostics
        .iter()
        .find(|row| row.warning_code.as_deref() == Some("cycle"))
        .ok_or("the actual typed class-cycle diagnostic")?;
    assert_eq!(
        diagnostic.count, count,
        "all source occurrences remain counted"
    );
    assert_eq!(
        diagnostic.examples.len(),
        1,
        "equal same-root examples merge their actual source recipes"
    );
    Ok(diagnostic
        .examples
        .first()
        .ok_or("the actual merged class example")?
        .body()?
        .clone())
}
