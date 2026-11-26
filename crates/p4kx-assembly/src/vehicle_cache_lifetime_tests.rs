// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_archive::Archive;
use p4kx_dataforge::DataForge;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry,
    TestPort, TestRecord, test_key,
};

use crate::{
    ArchiveFiles, ENTITY_TYPE, Entities, EntityNode, LoadoutFileObservation, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutRequest, LoadoutResolver, VehicleImplementation,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const BASE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/fleet.xml";
const VEHICLE: &[u8] = br#"<Vehicle>
    <Parts><Part name="Hull" class="MassBox" mass="15"/></Parts>
    <Modifications>
        <Modification name="Scoped" patchFile="Modifications/missing-fleet.xml"/>
    </Modifications>
</Vehicle>"#;

#[derive(Clone, Copy)]
enum Case {
    Complete,
    MissingPatch,
    Malformed,
    Missing,
}

#[derive(Default)]
struct Events {
    files: Mutex<FileEvents>,
    vehicle_users: Mutex<Vec<u32>>,
}

#[derive(Default)]
struct FileEvents {
    reads: Vec<LoadoutFileObservation>,
    reused: Vec<LoadoutFileObservation>,
}

impl LoadoutReadObserver for Events {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        let (original, read) = match event {
            LoadoutReadEvent::FileRead { observation, .. } => (observation, true),
            LoadoutReadEvent::FileReused { observation, .. } => (observation, false),
            _ => return ControlFlow::Continue(()),
        };
        let Ok(mut files) = self.files.lock() else {
            return ControlFlow::Break(());
        };
        if read {
            files.reads.push(original.clone());
        } else {
            files.reused.push(original.clone());
        }
        ControlFlow::Continue(())
    }

    fn observes_vehicle_totals(&self) -> bool {
        true
    }

    fn vehicle_used(
        &self,
        index: u32,
        _: &VehicleImplementation,
        _: &LoadoutRequest,
    ) -> ControlFlow<()> {
        let Ok(mut users) = self.vehicle_users.lock() else {
            return ControlFlow::Break(());
        };
        users.push(index);
        ControlFlow::Continue(())
    }
}

#[test]
fn one_use_fleet_owner_releases_successful_and_partial_vehicle_payloads() -> TestResult {
    for case in [Case::Complete, Case::MissingPatch] {
        with_fleet(
            case,
            [ship(1, "Unique", "unique", case)],
            |resolver, forge, events| {
                let resolved = resolve(resolver, forge, 0)?;
                let vehicle = resolved
                    .vehicle()
                    .ok_or("usable original vehicle implementation")?;
                assert_eq!(
                    vehicle.issues().is_empty(),
                    matches!(case, Case::Complete),
                    "a missing original patch remains explicit on a usable implementation"
                );
                let evidence = vehicle
                    .aggregate_evidence()
                    .ok_or("observed original aggregate")?;
                let files = events.files.lock().map_err(|_| "file observation lock")?;
                let original = files.reads.first().ok_or("original vehicle byte read")?;
                assert!(
                    original.source().entry_origin().is_some(),
                    "the fixture has a real Archive admission"
                );
                assert_eq!(
                    original.projection_outcome(),
                    Some(Ok(())),
                    "keep the original successful parse"
                );
                assert!(
                    evidence
                        .selectors()
                        .any(|site| site.file().is_same_operation(original)),
                    "the released payload keeps its original XML selector operations"
                );
                assert!(
                    files.reused.is_empty(),
                    "this physical root was resolved once"
                );
                assert!(
                    resolver.vehicles.is_empty(),
                    "successful one-use payload leaves the resolver cache"
                );
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn duplicate_physical_vehicle_keys_keep_exact_success_and_failure_operations() -> TestResult {
    for case in [
        Case::Complete,
        Case::MissingPatch,
        Case::Malformed,
        Case::Missing,
    ] {
        let roots = [
            ship(1, "Alias", "first", case),
            ship(1, "ALIAS", "second", case),
        ];
        with_fleet(case, roots, |resolver, forge, events| {
            let first = resolve(resolver, forge, 0)?;
            let second = resolve(resolver, forge, 1)?;
            assert_eq!(
                first.guid(),
                second.guid(),
                "fixture retains the same GUID on separate physical owners"
            );
            assert_eq!(
                first.vehicle(),
                second.vehicle(),
                "cached interpretation stays identical"
            );
            assert_eq!(
                resolver.vehicles.len(),
                1,
                "the possibly reused key remains cached"
            );
            assert_original_replay(events)?;
            let users = events
                .vehicle_users
                .lock()
                .map_err(|_| "vehicle observation lock")?;
            if first.vehicle().is_some() {
                assert_eq!(
                    *users,
                    [0, 1],
                    "the shared file binds to both physical callers"
                );
            } else {
                assert!(
                    users.is_empty(),
                    "failed originals never observe a usable vehicle"
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn normalized_fallback_names_prevent_an_incorrect_single_use_proof() -> TestResult {
    let raw_name = TestRecord::new(2, ENTITY_TYPE, "UNIQUE", "synthetic/other.xml");
    let roots = [ship(1, "Unique", "first", Case::Complete), raw_name];
    with_fleet(Case::Complete, roots, |resolver, forge, _| {
        assert!(
            resolve(resolver, forge, 0)?.vehicle().is_some(),
            "the prefixed root still resolves its vehicle"
        );
        assert_eq!(
            resolver.vehicles.len(),
            1,
            "bare fallback and prefixed classes normalize to the same key"
        );
        Ok(())
    })
}

#[test]
fn a_vehicle_record_reachable_beneath_another_root_keeps_its_original_operation() -> TestResult {
    let vehicle = TestEntity::new(3, "MountedVehicle", "synthetic/mount.xml")
        .attach_type("Other")
        .vehicle(BASE, "")
        .record();
    let roots = [host(1, "HostOne"), host(2, "HostTwo"), vehicle];
    with_fleet(Case::Complete, roots, |resolver, forge, events| {
        resolve(resolver, forge, 0)?;
        resolve(resolver, forge, 1)?;
        assert_eq!(
            resolver.vehicles.len(),
            1,
            "a non-docked nested vehicle may be reached again"
        );
        assert_original_replay(events)
    })
}

fn assert_original_replay(events: &Events) -> TestResult {
    let files = events.files.lock().map_err(|_| "file observation lock")?;
    assert!(
        !files.reads.is_empty(),
        "the first owner opens original candidates"
    );
    assert_eq!(
        files.reused.len(),
        files.reads.len(),
        "no new original read on a cache hit"
    );
    assert!(
        files
            .reads
            .iter()
            .zip(&files.reused)
            .all(|(read, reused)| read.is_same_operation(reused)),
        "each successive cache event reuses the exact original read or error in order"
    );
    Ok(())
}

fn ship(guid: u128, class: &str, file: &str, case: Case) -> TestRecord {
    let modification = if matches!(case, Case::MissingPatch) {
        "Scoped"
    } else {
        ""
    };
    TestEntity::new(guid, class, format!("synthetic/{file}.xml"))
        .attach_type("NOITEM_Vehicle")
        .vehicle(BASE, modification)
        .record()
}

fn host(guid: u128, name: &str) -> TestRecord {
    TestEntity::new(guid, name, format!("synthetic/{name}.xml"))
        .attach_type("NOITEM_Vehicle")
        .port(TestPort::new("mount"))
        .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::reference(
            "mount", 3,
        )]))
        .record()
}

fn resolve<'source>(
    resolver: &mut LoadoutResolver<'source>,
    forge: &'source DataForge<'source>,
    index: u32,
) -> TestResult<EntityNode> {
    Ok(resolver.resolve(&forge.record(index).ok_or("fixture Record")?)?)
}

fn with_fleet(
    case: Case,
    originals: impl IntoIterator<Item = TestRecord>,
    verify: impl for<'source> FnOnce(
        &mut LoadoutResolver<'source>,
        &'source DataForge<'source>,
        &Events,
    ) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    let mut builder = ArchiveBuilder::v2();
    match case {
        Case::Complete | Case::MissingPatch => {
            builder = builder.entry(TestEntry::stored(format!("Data/{BASE}"), VEHICLE).encrypted());
        }
        Case::Malformed => {
            builder = builder.entry(
                TestEntry::stored(format!("Data/{BASE}"), b"<broken>".as_slice()).encrypted(),
            );
        }
        Case::Missing => {}
    }
    builder.write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let files = ArchiveFiles::new(&archive);
    let database = originals
        .into_iter()
        .fold(
            p4kx_testkit::entity_schema(DataForgeBuilder::new()),
            DataForgeBuilder::record,
        )
        .build();
    let forge = DataForge::open(&database)?;
    let events = Arc::new(Events::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), events.clone())
            .with_files(&files)
            .stopping_at_docked_vehicles()
            .retiring_single_use_fleet_vehicles();
    verify(&mut resolver, &forge, &events)
}
