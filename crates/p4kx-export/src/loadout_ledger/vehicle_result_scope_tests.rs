// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{Entry, RecordTree, SourceValueState};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry, TestLoadout, entity_schema};

use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::ship_pool_test_input;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::contract::Ship;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::playability_rules::Evidence;
use crate::record_observation::RecordObservation;
use crate::ship::source_projection::{self, direct};
use crate::ship_pool::ShipPool;
use crate::texts::{TextSources, Texts};
use crate::{Kind, MassBasis};

const VEHICLE_PATH: &str = "Scripts/Entities/Vehicles/Implementations/Xml/ScopedShip.xml";
const LOADOUT_PATH: &str = "synthetic/scoped-ship-loadout.xml";

#[derive(Clone, Copy)]
enum VehicleFile {
    Readable,
    Unreadable,
    Missing,
}

#[test]
fn direct_vehicle_measurements_close_without_reading_the_damaged_loadout() -> TestResult {
    run(VehicleFile::Readable)
}

#[test]
fn direct_ship_uses_observed_physics_when_vehicle_xml_is_unavailable() -> TestResult {
    run(VehicleFile::Unreadable)?;
    run(VehicleFile::Missing)
}

fn run(case: VehicleFile) -> TestResult {
    let database = entity_schema(DataForgeBuilder::new().version(8))
        .record(
            TestEntity::new(
                1,
                "ScopedShip",
                "libs/foundry/records/entities/spaceships/scoped.xml",
            )
            .attach_type("NOITEM_Vehicle")
            .vehicle(VEHICLE_PATH, "")
            .physics_mass(17.0)
            .loadout(TestLoadout::Xml(LOADOUT_PATH.to_owned()))
            .record(),
        )
        .build();
    ship_pool_test_input::with_input(&database, entries(case), |_, files, _, input| {
        check_scope(files, input, case)
    })
}

fn entries(case: VehicleFile) -> Vec<TestEntry> {
    let corrupt = TestEntry::zstd(format!("Data/{LOADOUT_PATH}"), b"<Loadout/>".as_slice())
        .with_payload(vec![0, 1, 2]);
    let path = format!("Data/{VEHICLE_PATH}");
    let xml = b"<Vehicle><Parts><Part mass=\"12\" damageMax=\"8\"/></Parts></Vehicle>".as_slice();
    let mut entries = vec![corrupt];
    match case {
        VehicleFile::Readable => entries.push(TestEntry::stored(path, xml)),
        VehicleFile::Unreadable => {
            entries.push(TestEntry::zstd(path, xml).with_payload(vec![0, 1, 2]));
        }
        VehicleFile::Missing => {}
    }
    entries
}

fn check_scope<'source>(
    files: &'source ArchiveFiles<'source>,
    input: &'source OriginalCatalogInput<'source, 'source>,
    case: VehicleFile,
) -> TestResult {
    let forge = input.forge();
    let record = forge.record(0).ok_or("the exact original Ship Record")?;
    let tree = record.tree()?;
    let sources = TextSources::read_languages(files.archive(), &[])?;
    let texts = Texts::parse(&sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, input, &registry).for_kind(Kind::Ships);
    let admission = RecordObservation::capture(input, &record)?;
    let initial = crate::export_field_walk::inspect_observed(
        Some(input),
        &tree,
        &admission,
        Some(Kind::Ships),
        ExportSourceRole::Selected,
        None,
    )?;
    let (evidence, observed) = Evidence::read_for_ships([&tree], input, None)?;
    let mut ledger = ExportLedger::default();
    ledger.merge(observed);
    ledger.merge(initial);
    let direct = direct::read(
        files,
        &projection,
        input,
        &admission,
        &tree,
        &mut ledger,
        None,
    )?
    .map_err(|_| "the direct Ship resolves its original input")?;
    let playability =
        direct
            .source
            .playability_proof
            .decide(&tree, direct.source.vehicle, &evidence, input)?;
    let direct = Ship::from_read(direct.source, direct.vehicle, playability)?;
    verify_vehicle(&direct, &ledger, case);
    let (full, full_ledger) = full_row(files, input, &projection, &admission, &tree, &evidence)?;
    compare(files, &direct, &full, &ledger, &full_ledger)
}

fn full_row<'source>(
    files: &'source ArchiveFiles<'source>,
    input: &'source OriginalCatalogInput<'source, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    tree: &RecordTree<'source>,
    evidence: &Evidence,
) -> TestResult<(Ship<'source>, ExportLedger)> {
    let mut ledger = crate::export_field_walk::inspect_observed(
        Some(input),
        tree,
        admission,
        Some(Kind::Ships),
        ExportSourceRole::Selected,
        None,
    )?;
    let pool = ShipPool::new(Arc::new(Entities::new(input.forge())), files);
    let full =
        source_projection::read(&pool, projection, input, admission, tree, &mut ledger, None)?;
    let aggregates = full.aggregates.ok_or("the original full Ship aggregates")?;
    let read = full
        .read
        .map_err(|_| "the full Ship resolves its original input")?;
    let (ship, _, _) = crate::ship::ship(tree, full.source, evidence, input, &read, aggregates)?;
    Ok((ship, ledger))
}

fn compare(
    files: &ArchiveFiles<'_>,
    direct: &Ship<'_>,
    full: &Ship<'_>,
    ledger: &ExportLedger,
    full_ledger: &ExportLedger,
) -> TestResult {
    let path = format!("Data/{LOADOUT_PATH}");
    let loadout = files
        .archive()
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .map(Entry::index)
        .ok_or("the actual admitted damaged Loadout Entry")?;
    assert!(
        ledger.entries.iter().all(|entry| {
            entry.entry_index != Some(loadout)
                && !matches!(
                    entry.operation,
                    EntryOperation::XmlLoadout | EntryOperation::XmlLoadoutBytes
                )
        }),
        "the direct read makes no Loadout read or projection attempt"
    );
    assert!(
        full_ledger.entries.iter().any(|entry| {
            entry.entry_index == Some(loadout)
                && entry.operation == EntryOperation::XmlLoadoutBytes
                && entry.disposition == SourceDisposition::ReadFailed
        }),
        "the positive control reaches the same unreadable Loadout"
    );
    assert_eq!(
        serde_json::to_value(direct)?,
        serde_json::to_value(full)?,
        "the full canonical direct Ship row and its original source uses agree"
    );
    Ok(())
}

fn verify_vehicle(row: &Ship<'_>, ledger: &ExportLedger, case: VehicleFile) {
    let (mass, basis, hull, expected_operation) = match case {
        VehicleFile::Readable => (
            12.0,
            MassBasis::Parts,
            SourceValueState::Present,
            SourceDisposition::Projected,
        ),
        VehicleFile::Unreadable => (
            17.0,
            MassBasis::Physics,
            SourceValueState::Unreadable,
            SourceDisposition::ReadFailed,
        ),
        VehicleFile::Missing => (
            17.0,
            MassBasis::Physics,
            SourceValueState::Unreadable,
            SourceDisposition::SkippedBeforeRead,
        ),
    };
    assert_eq!(
        row.mass.value(),
        Some(&mass),
        "the selected original numeric mass"
    );
    assert_eq!(
        row.mass_basis.value(),
        Some(&basis),
        "the actual fallback decision"
    );
    assert_eq!(
        row.hull_health.state(),
        hull,
        "the selected hull source state"
    );
    assert_eq!(
        row.mass.origin().source_set_complete(),
        matches!(case, VehicleFile::Readable),
        "the unresolved selected implementation cannot establish a complete source set"
    );
    assert!(
        ledger.entries.iter().any(|entry| {
            entry.operation == EntryOperation::VehicleImplementationBytes
                && entry.disposition == expected_operation
        }),
        "the vehicle observer records the actual file read outcome"
    );
}
