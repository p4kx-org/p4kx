// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_extraction::{
    Archive, CancelToken, CanonicalReadObserver, DataForge, DataForgeEntry, DataForgeError, Record,
    RecordTree,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::{Family, Projector, Query, ReadOptions};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::item_stats::WEAPON_COMPONENT;
use crate::record_observation::RecordObservation;

#[derive(Default)]
struct Reads(AtomicUsize);

impl CanonicalReadObserver for Reads {
    fn observe(
        &self,
        _record: &Record<'_>,
        _result: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        self.0.fetch_add(1, Ordering::Relaxed);
        ControlFlow::Continue(())
    }
}

struct InterruptProjection;

impl Projector for InterruptProjection {
    type Row<'source> = ();
    const FAMILY: Family = Family::WeaponSources;

    fn read<'source>(
        _forge: &'source DataForge<'source>,
        _tree: &RecordTree<'source>,
        _projection: &CatalogProjection<'_, 'source>,
        _admission: &RecordObservation<'_, 'source>,
        _options: &ReadOptions<'_>,
        _cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        Err(CatalogError::Cancelled)
    }
}

#[test]
fn weapon_projection_stops_before_the_next_owner_body_read()
-> Result<(), Box<dyn std::error::Error>> {
    const ENTITY: &str = "EntityClassDefinition";
    const COMPONENT: &str = "DataForgeComponentParams";
    let mut database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(COMPONENT))
        .structure(TestStruct::new(WEAPON_COMPONENT).parent(COMPONENT))
        .structure(
            TestStruct::new(ENTITY)
                .array("Components", TestType::StrongPointer(COMPONENT.to_owned())),
        );
    for owner in 1..=2 {
        database = database.record(
            TestRecord::new(
                owner,
                ENTITY,
                format!("{ENTITY}.FixtureWeapon{owner}"),
                format!("libs/foundry/records/entities/scitem/fixture/weapon{owner}.xml"),
            )
            .value(
                "Components",
                TestValue::Array(vec![TestValue::Struct(TestInstance::new(WEAPON_COMPONENT))]),
            ),
        );
    }
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("weapon-owner-order.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let reads = Arc::new(Reads::default());
    let observer: Arc<dyn CanonicalReadObserver> = reads.clone();
    let outcome = Query {
        archive: &archive,
        entry: &entry,
        forge: &forge,
        selection: CatalogRecordSelection::All,
        languages: &[],
        cancel: None,
    }
    .run::<InterruptProjection, _>(Some(observer), |_| ());
    assert!(
        matches!(outcome, Err(CatalogError::Cancelled)),
        "the first selected owner reaches projection"
    );
    assert_eq!(
        reads.0.load(Ordering::Relaxed),
        1,
        "a failed projection stops before reading the next owner body"
    );
    Ok(())
}
