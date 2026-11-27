// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{ComponentOwner, EquipmentSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries, ProvenanceMode, ResourceFloat32,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, Build, DataForge, DataForgeEntry, RecordTree, SourceUses,
    SourceValueState, SourcedValue, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::{CONTROLLER_TYPE, EntryOrigin, IFCS_TYPE, Result, THRUSTER_TYPE, Trace, indexed};
use crate::real_archive_verification::print_line;

#[path = "flight_sdk_witness_selection.rs"]
mod selection;
use selection::{CONTROLLERS, IFCS, THRUSTERS, WitnessCase};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and select this test with --ignored"]
fn installed_flight_sdk_preserves_selected_original_components() -> Result {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let build = archive
        .build()
        .ok_or("an exact Archive Build is required")?;
    let identity = [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ];
    if identity.iter().any(|part| part.trim().is_empty()) {
        return Err("the Flight SDK witness requires all four Build identity fields".into());
    }
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let entry = DataForgeEntry::read(&archive)?;
    let original = entry.open()?;
    let selected = selection::discover(&original, &watch)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_provenance_mode(ProvenanceMode::Full);
    let queries = session.parsed(None)?;
    let sources = SourceUses::with_compact_coordinates();
    let original = Evidence {
        forge: &original,
        origin: EntryOrigin::read(&sources, session.dataforge_origin())?,
        build,
    };
    original.check(&queries, selected)?;
    watch.check()?;
    print_line(&format!(
        "Flight SDK source witness: {}; three selected component families",
        identity.join(" ")
    ));
    Ok(())
}

struct Evidence<'source> {
    forge: &'source DataForge<'source>,
    origin: EntryOrigin,
    build: &'source Build,
}

struct ComponentCells<'row, const N: usize> {
    owner: &'row ComponentOwner,
    concrete: Option<&'row str>,
    singles: [&'row SourcedValue<ResourceFloat32>; N],
}

impl Evidence<'_> {
    fn check(&self, queries: &ParsedCatalogQueries<'_>, owners: [u32; 3]) -> Result {
        let [controller, ifcs, thruster] = owners;
        queries.with_flight_controllers(
            CatalogRecordSelection::RecordIndex(controller),
            None,
            |snapshot| {
                self.verify(&snapshot, controller, CONTROLLERS, |row| ComponentCells {
                    owner: row.owner(),
                    concrete: row.source().source_type().value().copied(),
                    singles: [&row.settings.fuel_warning_display_time],
                })
            },
        )??;
        queries.with_ifcs(
            CatalogRecordSelection::RecordIndex(ifcs),
            None,
            |snapshot| {
                self.verify(&snapshot, ifcs, IFCS, |row| ComponentCells {
                    owner: row.owner(),
                    concrete: row.source().source_type().value().copied(),
                    singles: [&row.limits.max_speed],
                })
            },
        )??;
        queries.with_thrusters(
            CatalogRecordSelection::RecordIndex(thruster),
            None,
            |snapshot| {
                self.verify(&snapshot, thruster, THRUSTERS, |row| ComponentCells {
                    owner: row.owner(),
                    concrete: row.source().source_type().value().copied(),
                    singles: [
                        &row.operating.thrust_capacity,
                        &row.operating.thrust_capacity_new,
                    ],
                })
            },
        )??;
        Ok(())
    }

    fn verify<Row, const N: usize>(
        &self,
        snapshot: &EquipmentSnapshot<'_, Row>,
        index: u32,
        case: WitnessCase<N>,
        cells: impl for<'row> Fn(&'row Row) -> ComponentCells<'row, N>,
    ) -> Result {
        let tree = self
            .forge
            .record(index)
            .ok_or("selected physical Entity owner")?
            .tree()?;
        assert!(
            snapshot.build() == Some(self.build),
            "exact Build in SDK snapshot"
        );
        assert!(
            matches!(snapshot.availability(), DatasetAvailability::Available),
            "declared component family is available"
        );
        let coverage = snapshot.coverage();
        assert!(
            coverage.unreadable_headers.is_empty(),
            "the selected owner is classifiable"
        );
        assert!(
            coverage.unreadable_bodies.is_empty(),
            "the selected owner body is readable"
        );

        let inventory = serde_json::to_value(snapshot.source_inventory())?;
        let uses = indexed(&inventory, "/uses", "id")?;
        let trace = Trace::of(&tree, &uses, &self.origin.scope, self.origin.encrypted);
        verify_slots(&tree, snapshot, case, cells, &trace)
    }
}

fn verify_slots<'source, Row, const N: usize>(
    tree: &RecordTree<'source>,
    snapshot: &EquipmentSnapshot<'_, Row>,
    case: WitnessCase<N>,
    cells: impl for<'row> Fn(&'row Row) -> ComponentCells<'row, N>,
    trace: &Trace<'_, 'source>,
) -> Result {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("selected Entity has an original Components array".into());
    };
    let mut returned = snapshot.rows().iter();
    let mut positive = false;
    for (ordinal, raw) in components.iter().enumerate() {
        let Original::Struct(component) = raw else {
            continue;
        };
        if component.type_name() != case.kind {
            continue;
        }
        let row = returned
            .next()
            .ok_or("every matching original Components occurrence must be returned")?;
        let row = cells(row);
        assert!(
            row.concrete == Some(case.kind),
            "exact original component type"
        );
        let ordinal = u32::try_from(ordinal)?;
        assert!(
            row.owner
                .guid()
                .value()
                .is_some_and(|guid| guid.get() == tree.guid()),
            "original owner GUID"
        );
        physical_slot(row.owner, tree.record_index(), ordinal, raw, trace)?;
        for (field, output) in case.fields.iter().zip(row.singles) {
            positive |= single(output, component.field(field), trace)?;
        }
    }
    assert!(
        returned.next().is_none(),
        "the SDK result contains only the owner's matching slots"
    );
    assert!(
        positive,
        "the selected family contributed an original typed Single"
    );
    Ok(())
}

fn physical_slot<'source>(
    owner: &ComponentOwner,
    index: u32,
    ordinal: u32,
    raw: &Original<'source>,
    trace: &Trace<'_, 'source>,
) -> Result {
    let provided_index = owner.record_index();
    let provided_ordinal = owner.component_ordinal();
    assert!(
        provided_index.value() == Some(&index),
        "original physical owner index"
    );
    assert!(
        provided_ordinal.value() == Some(&ordinal),
        "original physical Components ordinal"
    );
    trace.header(
        &serde_json::to_value(provided_index)?,
        "recordIndex",
        "selector",
    )?;
    trace.header(&serde_json::to_value(owner.guid())?, "guid", "payload")?;
    trace.check(&serde_json::to_value(provided_ordinal)?, raw, "conversion")
}

fn single<'source>(
    output: &SourcedValue<ResourceFloat32>,
    original: Option<&Original<'source>>,
    trace: &Trace<'_, 'source>,
) -> Result<bool> {
    match original {
        Some(raw @ Original::Single(expected)) => {
            assert!(
                output.state() == SourceValueState::Present,
                "authored Single is present"
            );
            assert!(
                output.value().map(ResourceFloat32::raw_bits) == Some(expected.to_bits()),
                "exact original Single bits"
            );
            trace.check(&serde_json::to_value(output)?, raw, "payload")?;
            Ok(true)
        }
        Some(raw @ Original::Null) => {
            assert!(
                output.state() == SourceValueState::Null,
                "authored null retains its state"
            );
            trace.check(&serde_json::to_value(output)?, raw, "selector")?;
            Ok(false)
        }
        None => {
            assert!(
                output.state() == SourceValueState::Absent,
                "absent original field retains its state"
            );
            Ok(false)
        }
        _ => Err("the selected witness field must be an original Single or null".into()),
    }
}
