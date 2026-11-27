// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Loadout edge cases: cycles, depth, wide trees, overrides, port defaults and References
//! to Records that are no Entities.

use std::error::Error;

use p4kx_assembly::{
    Attachment, EntityNode, LoadoutError, LoadoutOrigin, LoadoutResolver, MAX_LOADOUT_DEPTH,
    PortSource,
};
use p4kx_dataforge::DataForge;
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestLoadout, TestLoadoutEntry, TestPort, TestRecord, TestStruct,
    entity_schema,
};

type TestResult = Result<(), Box<dyn Error>>;

const A: u128 = 0x1000;
const B: u128 = 0x2000;
const C: u128 = 0x3000;

fn database(entities: &[TestEntity]) -> DataForgeBuilder {
    entities
        .iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        })
}

fn resolve(builder: &DataForgeBuilder, class_name: &str) -> Result<EntityNode, Box<dyn Error>> {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    let entity = resolver.entities().find(class_name)?;
    Ok(resolver.resolve(&entity)?)
}

fn entity(guid: u128, class_name: &str) -> TestEntity {
    TestEntity::new(guid, class_name, format!("entities/{class_name}.xml"))
}

const fn manual(entries: Vec<TestLoadoutEntry>) -> TestLoadout {
    TestLoadout::Manual(entries)
}

fn child(node: &EntityNode, port: &str) -> Result<EntityNode, Box<dyn Error>> {
    let found = node
        .ports()
        .iter()
        .find(|p| p.name() == port)
        .ok_or_else(|| port.to_owned())?;
    match found.attachment() {
        Attachment::Entity(child) => Ok(*child.clone()),
        other => Err(format!("{port} holds {other:?}").into()),
    }
}

fn error_at(node: &EntityNode, port: &str) -> Result<LoadoutError, Box<dyn Error>> {
    let found = node
        .ports()
        .iter()
        .find(|p| p.name() == port)
        .ok_or_else(|| port.to_owned())?;
    match found.attachment() {
        Attachment::Unresolved(unresolved) => Ok(unresolved.error().clone()),
        other => Err(format!("{port} holds {other:?}").into()),
    }
}

#[test]
fn an_entity_that_holds_itself_further_up_is_a_cycle_not_an_endless_tree() -> TestResult {
    let builder = database(&[
        entity(A, "A").loadout(manual(vec![TestLoadoutEntry::class("to_b", "B")])),
        entity(B, "B").loadout(manual(vec![
            TestLoadoutEntry::reference("to_a", A),
            TestLoadoutEntry::class("to_c", "C"),
        ])),
        entity(C, "C").loadout(manual(vec![TestLoadoutEntry::class("to_c", "C")])),
    ]);
    let root = resolve(&builder, "A")?;
    let b = child(&root, "to_b")?;
    assert!(
        matches!(error_at(&b, "to_a")?, LoadoutError::Cycle { .. }),
        "B holds A, which holds B"
    );
    let c = child(&b, "to_c")?;
    assert_eq!(error_at(&c, "to_c")?.kind(), "cycle", "C holds itself");
    Ok(())
}

#[test]
fn a_chain_deeper_than_the_limit_ends_in_too_deep() -> TestResult {
    let count = MAX_LOADOUT_DEPTH + 5;
    let entities: Vec<TestEntity> = (0..count)
        .map(|index| {
            let guid = 0x10_0000 + u128::try_from(index).unwrap_or_default();
            entity(guid, &format!("Link{index}")).loadout(manual(vec![TestLoadoutEntry::class(
                "next",
                format!("Link{}", index + 1),
            )]))
        })
        .collect();
    let mut node = resolve(&database(&entities), "Link0")?;
    for _ in 0..MAX_LOADOUT_DEPTH {
        node = child(&node, "next")?;
    }
    assert_eq!(error_at(&node, "next")?, LoadoutError::TooDeep, "the limit");
    Ok(())
}

#[test]
fn a_wide_loadout_retains_every_attached_branch_in_order() -> TestResult {
    const FAN: usize = 256;
    let ports = |class_name: Option<&str>| {
        (0..FAN)
            .map(|index| {
                let port = format!("p{index}");
                class_name.map_or_else(
                    || TestLoadoutEntry::empty(port.clone()),
                    |name| TestLoadoutEntry::class(port.clone(), name),
                )
            })
            .collect()
    };
    let builder = database(&[
        entity(A, "Hub").loadout(manual(ports(Some("Leaf")))),
        entity(B, "Leaf").loadout(manual(ports(None))),
    ]);
    let root = resolve(&builder, "Hub")?;
    let mut observed = 0;
    root.visit_ports(|_, _| observed += 1);
    assert_eq!(observed, FAN * (FAN + 1), "every parent and child port");
    assert_eq!(root.ports().len(), FAN, "all authored attachments");
    assert!(root.issues().is_empty(), "the complete root has no issue");
    let tail = root.ports().last().ok_or("the last parent port")?;
    assert_eq!(tail.name(), "p255", "authored parent order");
    let Attachment::Entity(leaf) = tail.attachment() else {
        return Err("the last port retains its attached Entity".into());
    };
    assert!(leaf.issues().is_empty(), "the last leaf is complete");
    let last = leaf.ports().last().ok_or("the last leaf port")?;
    assert_eq!(last.name(), "p255", "authored child order");
    assert!(
        matches!(last.origin(), Some(LoadoutOrigin::Record(path)) if path.as_ref() == leaf.record_path()),
        "the last port keeps the original child Record",
    );
    Ok(())
}

#[test]
fn an_ambiguous_class_name_attaches_the_first_entity_and_says_so() -> TestResult {
    let builder = database(&[
        entity(A, "Ship").loadout(manual(vec![TestLoadoutEntry::class("seat", "SEAT")])),
        TestEntity::new(B, "Seat", "entities/seat_one.xml"),
        TestEntity::new(C, "Seat", "entities/seat_two.xml"),
    ]);
    let seat = child(&resolve(&builder, "Ship")?, "seat")?;
    assert_eq!(
        seat.record_path(),
        "entities/seat_one",
        "the first in definition order"
    );
    let issue = seat.issues().first().ok_or("no issue")?;
    assert_eq!(issue.kind(), "ambiguous_class", "kind");
    assert_eq!(
        issue.to_string(),
        "2 Entities have the class name SEAT; the first is attached: \
         00000000-0000-0000-0000-000000002000 (entities/seat_one), \
         00000000-0000-0000-0000-000000003000 (entities/seat_two)",
        "reason"
    );
    Ok(())
}
#[test]
fn an_entry_loadout_overrides_the_attached_entity_s_own_per_port() -> TestResult {
    let builder = database(&[
        entity(A, "Ship").loadout(manual(vec![
            TestLoadoutEntry::class("turret", "Turret").loadout(manual(vec![
                TestLoadoutEntry::class("gun_left", "Laser"),
                TestLoadoutEntry::empty("gun_right"),
                TestLoadoutEntry::class("extra", "Laser"),
            ])),
        ])),
        entity(B, "Turret").loadout(manual(vec![
            TestLoadoutEntry::class("gun_left", "Cannon"),
            TestLoadoutEntry::class("gun_right", "Cannon"),
            TestLoadoutEntry::class("seat", "Cannon"),
        ])),
        entity(C, "Laser"),
        entity(0x4000, "Cannon"),
    ]);
    let turret = child(&resolve(&builder, "Ship")?, "turret")?;
    let ports: Vec<(&str, PortSource, Option<String>)> = turret
        .ports()
        .iter()
        .map(|port| {
            let held = match port.attachment() {
                Attachment::Entity(node) => Some(node.class_name().to_owned()),
                _ => None,
            };
            (port.name(), port.source(), held)
        })
        .collect();
    assert_eq!(
        ports,
        [
            ("gun_left", PortSource::Override, Some("Laser".to_owned())),
            ("gun_right", PortSource::Override, None),
            ("seat", PortSource::Loadout, Some("Cannon".to_owned())),
            ("extra", PortSource::Override, Some("Laser".to_owned())),
        ],
        "overridden in place, kept, then appended"
    );
    Ok(())
}

#[test]
fn a_port_no_loadout_names_holds_its_default_item() -> TestResult {
    let builder = database(&[
        entity(A, "Table")
            .port(TestPort::new("slot_glass").default_item(B))
            .port(TestPort::new("slot_named").default_item(B))
            .loadout(manual(vec![TestLoadoutEntry::empty("slot_named")])),
        entity(B, "Glass"),
    ]);
    let table = resolve(&builder, "Table")?;
    let glass = table.ports().get(1).ok_or("no second port")?;
    assert_eq!(glass.name(), "slot_glass", "after the Loadout ports");
    assert_eq!(glass.source(), PortSource::PortDefault, "source");
    assert_eq!(
        child(&table, "slot_glass")?.class_name(),
        "Glass",
        "default"
    );
    let named = table.ports().first().ok_or("no first port")?;
    assert_eq!(
        named.attachment(),
        &Attachment::Empty,
        "a Loadout entry wins over the default item"
    );
    Ok(())
}

#[test]
fn a_reference_to_a_record_that_is_no_entity_is_reported() -> TestResult {
    let builder = database(&[
        entity(A, "Ship").loadout(manual(vec![TestLoadoutEntry::reference("tag", 0x9999)]))
    ])
    .structure(TestStruct::new("Tag"))
    .record(TestRecord::new(
        0x9999,
        "Tag",
        "Tag.Shiny",
        "tags/shiny.xml",
    ));
    let ship = resolve(&builder, "Ship")?;
    let error = error_at(&ship, "tag")?;
    assert_eq!(error.kind(), "not_an_entity", "kind: {error}");
    assert!(error.to_string().contains("is a Tag"), "reason: {error}");
    Ok(())
}

#[test]
fn resolving_twice_with_one_resolver_gives_the_same_tree() -> TestResult {
    let bytes = p4kx_testkit::sample_ship().build();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    let ship = resolver.entities().find("test_ship")?;
    let first = resolver.resolve(&ship)?;
    let second = resolver.resolve(&ship)?;
    assert_eq!(first, second, "cached Records give the same tree");
    let fresh = LoadoutResolver::new(&forge).resolve(&ship)?;
    assert_eq!(first, fresh, "a new resolver gives the same tree");
    Ok(())
}
