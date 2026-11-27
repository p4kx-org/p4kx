// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::sync::Arc;
use std::thread;

use p4kx_dataforge::{DataForge, Guid, Record};
use p4kx_testkit::{DataForgeBuilder, TestEntity, entity_schema};

use super::{Deferred, LoadoutResolver};
use crate::LoadoutReference;
use crate::manual_loadout::ManualLoadout;
use crate::spec::{EntrySpec, LoadoutOrigin, LoadoutSpec, TargetSpec};

const DEEP: u128 = 1;
const DOCKED: u128 = 2;
const EXCLUDED: u128 = 3;
const ORDINARY: u128 = 4;
const DEFERRED: u128 = 5;
const SIBLING: u128 = 6;

#[test]
fn nested_prefetch_preserves_order_and_docked_boundaries_on_a_small_stack() -> io::Result<()> {
    const LEVELS: usize = 4_096;
    let bytes = database().build();
    let mut retained = Vec::with_capacity(LEVELS);
    let mut nested = list(entry(Some(DEEP), None));
    for _ in 0..LEVELS {
        retained.push(nested.clone());
        nested = list(entry(None, Some(nested)));
    }
    let result = thread::scope(|scope| {
        thread::Builder::new()
            .stack_size(128 << 10)
            .spawn_scoped(scope, || verify_prefetch(&bytes, &nested))?
            .join()
            .map_err(|_| io::Error::other("the small-stack prefetch worker panicked"))?
    });
    drop(nested);
    // Each child list stays shared until its parent has been released.
    while let Some(level) = retained.pop() {
        drop(level);
    }
    result
}

fn verify_prefetch(bytes: &[u8], nested: &ManualLoadout) -> io::Result<()> {
    let forge = DataForge::open(bytes).map_err(|error| io::Error::other(error.to_string()))?;
    let mut resolver = LoadoutResolver::new(&forge).stopping_at_docked_vehicles();
    let entries = [
        entry(None, Some(nested.clone())),
        entry(Some(DOCKED), Some(list(entry(Some(EXCLUDED), None)))),
        entry(Some(ORDINARY), Some(list(entry(Some(DEFERRED), None)))),
        entry(Some(SIBLING), None),
    ];
    let mut deferred = Vec::new();
    check_records(
        &resolver,
        &entries,
        &mut deferred,
        [DEEP, DOCKED, ORDINARY, SIBLING],
    );
    assert_eq!(
        deferred
            .iter()
            .map(|(target, _)| *target)
            .collect::<Vec<_>>(),
        [guid(DOCKED), guid(ORDINARY)],
        "unread targets defer their nested lists in source order",
    );
    for target in [DOCKED, ORDINARY] {
        let record = forge
            .record_by_guid(guid(target))
            .ok_or_else(|| io::Error::other("fixture owner"))?;
        let spec = resolver.spec_record(&record);
        assert!(spec.as_ref().is_ok(), "the fixture owner can be prefetched");
    }
    let resumed = resolver.unread_targets(&[], &mut deferred);
    assert_eq!(
        resumed.iter().map(Record::guid).collect::<Vec<_>>(),
        [guid(DEFERRED)],
        "only the nested list of the non-docked owner resumes",
    );
    assert!(deferred.is_empty(), "the deferred lists were handled");
    check_records(
        &resolver,
        &entries,
        &mut deferred,
        [DEEP, DOCKED, ORDINARY, DEFERRED, SIBLING],
    );
    assert!(
        deferred.is_empty(),
        "cached owners need no further deferral"
    );
    Ok(())
}

fn check_records<const COUNT: usize>(
    resolver: &LoadoutResolver<'_>,
    entries: &[EntrySpec],
    deferred: &mut Vec<Deferred>,
    expected: [u128; COUNT],
) {
    let mut records = Vec::new();
    resolver.entry_records(entries, &mut records, deferred);
    assert_eq!(
        records.iter().map(Record::guid).collect::<Vec<_>>(),
        expected.map(guid),
        "the scanner emits exactly the physically reached owners in depth-first order",
    );
}

fn guid(value: u128) -> Guid {
    // DataForge stores each 64-bit half in little-endian order.
    Guid::from_stored(value.rotate_left(u64::BITS).to_le_bytes())
}

fn database() -> DataForgeBuilder {
    let owners = [
        (DEEP, "Deep", "Misc"),
        (DOCKED, "Docked", "NOITEM_Vehicle"),
        (EXCLUDED, "Excluded", "Misc"),
        (ORDINARY, "Ordinary", "Misc"),
        (DEFERRED, "Deferred", "Misc"),
        (SIBLING, "Sibling", "Misc"),
    ];
    owners.into_iter().fold(
        entity_schema(DataForgeBuilder::new()),
        |builder, (id, name, kind)| {
            builder.record(
                TestEntity::new(id, name, format!("entities/{name}.xml"))
                    .attach_type(kind)
                    .record(),
            )
        },
    )
}

fn list(entry: EntrySpec) -> ManualLoadout {
    ManualLoadout::new(Arc::from([entry]))
}

fn entry(target: Option<u128>, nested: Option<ManualLoadout>) -> EntrySpec {
    EntrySpec {
        origin: LoadoutOrigin::Record(Arc::from("fixture/loadout")),
        port: String::from("fixture"),
        port_proof: None,
        target: target.map_or(TargetSpec::None, |target| {
            TargetSpec::Reference(LoadoutReference::unbound(guid(target)))
        }),
        loadout: nested.map(LoadoutSpec::Manual),
        xml_route: None,
        xml_use: None,
    }
}
