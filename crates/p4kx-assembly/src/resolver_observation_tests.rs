// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_dataforge::{DataForge, Value};
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestLoadout, TestRecord, TestStruct, TestType, TestValue,
    entity_schema,
};

use crate::{Entities, LoadoutError, LoadoutReadEvent, LoadoutReadObserver, LoadoutResolver};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ROOT: u128 = 0x6b00_0000_0000_4000_8000_0000_0000_0001;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Seen {
    Read(u32, bool, bool),
    Provided(u32),
    Reused(u32),
    Rejected(u32),
}

#[derive(Default)]
struct Observer {
    seen: Mutex<Vec<Seen>>,
    stop: bool,
}

impl Observer {
    fn events(&self) -> Vec<Seen> {
        let Ok(seen) = self.seen.lock() else {
            panic!("observer evidence mutex is poisoned");
        };
        seen.clone()
    }
}

impl LoadoutReadObserver for Observer {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        let event = match event {
            LoadoutReadEvent::FileRead { .. }
            | LoadoutReadEvent::FileProjection { .. }
            | LoadoutReadEvent::FileReused { .. }
            | LoadoutReadEvent::InventoryIncomplete => {
                return ControlFlow::Continue(());
            }
            LoadoutReadEvent::CanonicalRead { record, outcome } => Seen::Read(
                record.index(),
                outcome.is_ok(),
                outcome.is_ok_and(|tree| matches!(tree.field("bad"), Some(Value::Error(_)))),
            ),
            LoadoutReadEvent::ProvidedTree { record, .. } => Seen::Provided(record.index()),
            LoadoutReadEvent::Reused { record } => Seen::Reused(record.index()),
            LoadoutReadEvent::Rejected { record, .. } => Seen::Rejected(record.index()),
        };
        let Ok(mut seen) = self.seen.lock() else {
            panic!("observer evidence mutex is poisoned");
        };
        seen.push(event);
        drop(seen);
        if self.stop {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}

fn simple() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(TestStruct::new("EntityClassDefinition").field("bad", TestType::Bool))
}

fn entity(name: &str) -> TestRecord {
    TestRecord::new(
        ROOT,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{name}"),
        format!("synthetic/{name}.xml"),
    )
}

#[test]
fn successful_canonical_observation_precedes_optional_helpers_and_cache_reuse() -> TestResult {
    let bytes = simple()
        .record(entity("first").value("bad", TestValue::Raw(vec![2])))
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("root missing")?;
    let observer = Arc::new(Observer::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone());
    let node = resolver.resolve(&record)?;
    assert!(
        node.ports().is_empty(),
        "the explicit zero-port projection remains successful"
    );
    resolver.resolve(&record)?;
    let seen = observer.events();
    assert_eq!(
        seen.iter()
            .filter(|event| matches!(event, Seen::Read(..)))
            .count(),
        1,
        "a cached source is not decoded again"
    );
    assert!(
        seen.contains(&Seen::Read(0, true, true)),
        "actual Value::Error leaves reach the observer before projection helpers"
    );
    assert!(
        seen.contains(&Seen::Reused(0)),
        "cache reuse retains its real physical source identity"
    );
    Ok(())
}

#[test]
fn caller_provided_roots_do_not_trigger_an_additional_canonical_decode() -> TestResult {
    let bytes = simple().record(entity("provided")).build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("root missing")?;
    let tree = record.tree()?;
    let observer = Arc::new(Observer::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone());
    resolver.resolve_tree(&record, &tree)?;
    let seen = observer.events();
    assert!(
        seen.contains(&Seen::Provided(0)),
        "the supplied immutable tree is explicitly distinguished from a decoder call"
    );
    assert!(
        !seen.iter().any(|event| matches!(event, Seen::Read(..))),
        "the provided root is not decoded twice"
    );
    Ok(())
}

fn verify_alias_phase(events: &[Seen], record_index: u32, expected_reads: usize) {
    assert!(
        events.iter().all(|event| matches!(
            event,
            Seen::Read(source, true, false) | Seen::Reused(source) if *source == record_index
        )),
        "every observed operation in this resolve belongs to the exact selected physical alias"
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Seen::Read(..)))
            .count(),
        expected_reads,
        "each physical source is decoded exactly once and cached resolves never decode it again"
    );
    if expected_reads == 0 {
        assert!(
            events.contains(&Seen::Reused(record_index)),
            "a cached resolve retains its actual physical source provenance"
        );
    }
}

#[test]
fn guid_aliases_have_independent_physical_reads_and_cache_reuse() -> TestResult {
    let bytes = simple()
        .record(entity("first"))
        .record(entity("second"))
        .build();
    let forge = DataForge::open(&bytes)?;
    let first = forge.record(0).ok_or("first alias missing")?;
    let second = forge.record(1).ok_or("second alias missing")?;
    assert_eq!(
        first.guid(),
        second.guid(),
        "the fixture requires shared GUID aliases"
    );
    let observer = Arc::new(Observer::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone());
    for (record, expected_reads) in [(first, 1), (second, 1), (first, 0), (second, 0)] {
        let before = observer.events().len();
        resolver.resolve(&record)?;
        let events = observer.events();
        let phase = events
            .get(before..)
            .ok_or("alias observation phase is absent")?;
        verify_alias_phase(phase, record.index(), expected_reads);
    }
    Ok(())
}

#[test]
fn observer_stop_propagates_without_falling_back_to_another_root_decode() -> TestResult {
    let bytes = simple().record(entity("stopped")).build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("root missing")?;
    let observer = Arc::new(Observer {
        stop: true,
        ..Observer::default()
    });
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone());
    assert!(
        matches!(
            resolver.resolve(&record),
            Err(LoadoutError::ObservationStopped)
        ),
        "the observer's stop reaches the caller"
    );
    assert!(
        matches!(
            resolver.resolve(&record),
            Err(LoadoutError::ObservationStopped)
        ),
        "an already stopped observer does not resume through a cache miss"
    );
    assert_eq!(
        observer.events(),
        [Seen::Read(0, true, false)],
        "one actual decode is observed and no second decode occurs"
    );
    Ok(())
}

#[test]
fn canonical_failures_remain_typed_read_failures() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("EntityClassDefinition")
                .field("value", TestType::Int32)
                .declared_size(6),
        )
        .record(entity("broken"))
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("root missing")?;
    let observer = Arc::new(Observer::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone());
    assert!(
        matches!(
            resolver.resolve(&record),
            Err(LoadoutError::UnreadableRecord { .. })
        ),
        "the original loadout read error remains typed"
    );
    assert!(
        observer.events().contains(&Seen::Read(0, false, false)),
        "the actual canonical failure is observable before string conversion"
    );
    Ok(())
}

#[test]
fn type_rejections_are_not_fabricated_canonical_reads() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Other"))
        .record(TestRecord::new(
            ROOT,
            "Other",
            "Other.Synthetic",
            "synthetic/other.xml",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("source missing")?;
    let observer = Arc::new(Observer::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone());
    assert!(
        matches!(
            resolver.resolve(&record),
            Err(LoadoutError::NotAnEntity { .. })
        ),
        "the known wrong dependency type is rejected"
    );
    let seen = observer.events();
    assert!(
        seen.contains(&Seen::Rejected(0)),
        "pre-read rejection carries the actual physical source"
    );
    assert!(
        !seen.iter().any(|event| matches!(event, Seen::Read(..))),
        "no canonical read is invented for a rejected type"
    );
    Ok(())
}

#[test]
fn assortment_and_landing_gear_reads_share_the_observer() -> TestResult {
    let bytes = entity_schema(DataForgeBuilder::new())
        .structure(TestStruct::new("SLoadoutAssortment"))
        .record(
            TestEntity::new(ROOT, "Root", "synthetic/root.xml")
                .loadout(TestLoadout::RandomGenerated(vec![ROOT + 1]))
                .vehicle("", "")
                .landing_system(ROOT + 2)
                .record(),
        )
        .record(TestRecord::new(
            ROOT + 1,
            "SLoadoutAssortment",
            "SLoadoutAssortment.Synthetic",
            "synthetic/assortment.xml",
        ))
        .record(TestRecord::new(
            ROOT + 2,
            "VehicleLandingGearSystem",
            "VehicleLandingGearSystem.Synthetic",
            "synthetic/landing.xml",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("root missing")?;
    let observer = Arc::new(Observer::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone());
    resolver.resolve(&record)?;
    resolver.resolve(&record)?;
    let seen = observer.events();
    for index in [0, 1, 2] {
        assert_eq!(
            seen.iter()
                .filter(|event| matches!(event, Seen::Read(found, true, _) if *found == index))
                .count(),
            1,
            "the actual source at physical index {index} is read exactly once"
        );
        assert!(
            seen.contains(&Seen::Reused(index)),
            "cache reuse of physical source {index} is observable"
        );
    }
    Ok(())
}

#[path = "resolver_alias_tests.rs"]
mod aliases;
