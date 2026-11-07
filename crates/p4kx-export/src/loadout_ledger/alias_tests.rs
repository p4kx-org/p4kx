// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{
    Arc, Capture, CatalogGuid, DataForge, DataForgeBuilder, Entities, EntityKind, LoadoutResolver,
    PortReader, ROOT, TestEntity, TestResult, Wanted,
};

#[test]
fn wanted_uses_the_actual_physical_item_instead_of_its_guid_alias() -> TestResult {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(ROOT, "First", "synthetic/first.xml")
                .port(p4kx_testkit::TestPort::new("first"))
                .record(),
        )
        .record(
            TestEntity::new(ROOT, "Second", "synthetic/second.xml")
                .port(p4kx_testkit::TestPort::new("second"))
                .record(),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(1).ok_or("second physical source missing")?;
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
    .ok_or("loadout missing")?;
    assert_eq!(
        row.ports.first().ok_or("port missing")?.name,
        "second",
        "the final loadout derives from its exact original Item source"
    );
    let ledger = capture.finish()?;
    assert!(
        ledger.sources.iter().all(|source| source.record_index == 1),
        "no canonical first-GUID alias receives fabricated dependency provenance"
    );
    Ok(())
}

#[test]
fn a_wanted_index_with_a_different_guid_fails_accounting_without_a_fake_read() -> TestResult {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(TestEntity::new(ROOT, "Root", "synthetic/root.xml").record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("source missing")?;
    let tree = record.tree()?;
    let capture = Arc::new(Capture::new(&super::source(&record, &tree)?, None));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone());
    let wrong_guid = p4kx_extraction::Guid::from_stored([0xff; 16]);
    let row = crate::loadout::read_with(
        &mut resolver,
        &mut PortReader::new(),
        &forge,
        Wanted {
            record_index: record.index(),
            guid: CatalogGuid(wrong_guid),
            kind: EntityKind::Item,
        },
        &capture,
    )
    .map_err(|failure| failure.reason)?;
    assert!(
        row.is_none(),
        "no row is fabricated for a mismatched selected identity"
    );
    assert!(
        matches!(capture.finish(), Err(crate::CatalogError::Accounting(_))),
        "provenance disagreement prevents publication rather than silently using another source"
    );
    Ok(())
}
