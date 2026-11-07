// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{
    Arc, Capture, CatalogGuid, DataForge, DataForgeBuilder, Entities, EntityKind,
    ExportDiagnosticSeverity, ExportSourceRole, LoadoutResolver, PortReader, ROOT,
    SourceDisposition, TestEntity, TestLoadout, TestLoadoutEntry, TestResult, Wanted,
};

#[test]
fn a_dangling_attachment_retains_its_target_without_becoming_unreadable_source() -> TestResult {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(ROOT, "Root", "synthetic/root.xml")
                .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::reference(
                    "slot",
                    ROOT + 1,
                )]))
                .record(),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("source missing")?;
    let tree = record.tree()?;
    let capture = Arc::new(Capture::new(&super::source(&record, &tree)?, None));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone());
    let row = crate::loadout::read_with(
        &mut resolver,
        &mut PortReader::new(),
        &forge,
        Wanted {
            record_index: record.index(),
            guid: CatalogGuid(record.guid()),
            kind: EntityKind::Item,
        },
        &capture,
    )
    .map_err(|failure| failure.reason)?
    .ok_or("represented port row missing")?;
    let target = row
        .ports
        .first()
        .ok_or("port missing")?
        .item_guid
        .ok_or("target identity missing")?
        .0
        .to_string();
    let ledger = capture.finish()?;
    let note = ledger
        .field_diagnostics
        .iter()
        .find(|diagnostic| diagnostic.warning_code.as_deref() == Some("dangling_reference"))
        .ok_or("typed dangling diagnostic missing")?;
    assert_eq!(
        note.severity,
        ExportDiagnosticSeverity::Information,
        "a represented missing target is not a decoder failure"
    );
    assert_eq!(
        note.examples
            .first()
            .ok_or("example missing")?
            .target_guid
            .as_deref(),
        Some(target.as_str()),
        "the exact configured target identity remains exportable"
    );
    Ok(())
}

#[test]
fn an_actual_unavailable_xml_dependency_is_a_warning_without_omitting_the_ship() -> TestResult {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(ROOT, "Root", "synthetic/root.xml")
                .loadout(TestLoadout::Xml("synthetic/unavailable.xml".to_owned()))
                .record(),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("source missing")?;
    let tree = record.tree()?;
    let capture = Arc::new(Capture::new(&super::source(&record, &tree)?, None));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone());
    let row = crate::loadout::read_with(
        &mut resolver,
        &mut PortReader::new(),
        &forge,
        Wanted {
            record_index: record.index(),
            guid: CatalogGuid(record.guid()),
            kind: EntityKind::Ship,
        },
        &capture,
    )
    .map_err(|failure| failure.reason)?;
    assert!(
        row.is_some(),
        "the existing retained Ship/loadout representation remains available for diagnostic export"
    );
    let ledger = capture.finish()?;
    assert!(
        ledger
            .field_diagnostics
            .iter()
            .any(
                |diagnostic| diagnostic.warning_code.as_deref() == Some("no_loadout_files")
                    && diagnostic.severity == ExportDiagnosticSeverity::Warning
            ),
        "the actual unfulfilled external dependency blocks normal complete publication"
    );
    assert!(
        ledger
            .sources
            .iter()
            .filter(|source| source.role == ExportSourceRole::DerivedInput)
            .all(|source| source.disposition == SourceDisposition::Projected),
        "a retained partial representation does not fabricate an omitted original source Record"
    );
    Ok(())
}

#[test]
fn many_represented_targets_keep_complete_identity_and_bounded_examples() -> TestResult {
    let entries = (0_u32..50)
        .map(|index| {
            TestLoadoutEntry::reference(format!("slot{index}"), ROOT + u128::from(index + 1))
        })
        .collect();
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(ROOT, "Root", "synthetic/root.xml")
                .loadout(TestLoadout::Manual(entries))
                .record(),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("source missing")?;
    let tree = record.tree()?;
    let capture = Arc::new(Capture::new(&super::source(&record, &tree)?, None));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone());
    crate::loadout::read_with(
        &mut resolver,
        &mut PortReader::new(),
        &forge,
        Wanted {
            record_index: record.index(),
            guid: CatalogGuid(record.guid()),
            kind: EntityKind::Item,
        },
        &capture,
    )
    .map_err(|failure| failure.reason)?;
    let ledger = capture.finish()?;
    let note = ledger
        .field_diagnostics
        .iter()
        .find(|diagnostic| diagnostic.warning_code.as_deref() == Some("dangling_reference"))
        .ok_or("typed target diagnostic missing")?;
    assert_eq!(
        note.count, 50,
        "the stream counts actual typed unresolved-attachment occurrences"
    );
    assert_eq!(
        note.examples.len(),
        crate::EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT,
        "examples are bounded independently of the total"
    );
    assert_eq!(
        ledger
            .field_inspections
            .first()
            .ok_or("actual canonical proof missing")?
            .unresolved_references
            .len(),
        50,
        "bounded examples never discard the complete configured target identities"
    );
    Ok(())
}
