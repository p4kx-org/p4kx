// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{DataForge, TreeBuilder, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{AMMO_CONTAINER, PRIMARY_MAGAZINE_FIELD, WEAPON_COMPONENT, read, read_scoped};
use crate::build::original_input::test_support;
use crate::catalog_read::original::OriginalFlowFields;
use crate::item_variant::{self, ItemComponentKey, ItemKey};
use crate::record_fields::component;

type TestResult = Result<(), Box<dyn Error>>;

const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SAttachableComponentParams";
const ATTACH_DEF: &str = "SItemDefinition";
const AMMO: &str = "AmmoParams";
const MAGAZINE_GUID: u128 = 11;
const FIRST_MAGAZINE: &str = "FixtureMagazineFirst";
const SECOND_MAGAZINE: &str = "FixtureMagazineSecond";

#[path = "variant_cohort_tests.rs"]
mod variant_cohort_tests;

#[test]
fn physical_magazine_aliases_produce_distinct_item_variant_stats() -> TestResult {
    let bytes = fixture(
        TestType::WeakPointer(ENTITY.to_owned()),
        TestValue::pointer(SECOND_MAGAZINE),
        TestValue::pointer(FIRST_MAGAZINE),
    )
    .build();
    let forge = DataForge::open(&bytes)?;
    let mut trees = TreeBuilder::new();
    let weak = read_item(&forge, &mut trees, 0)?;
    let first = read_item(&forge, &mut trees, 1)?;
    assert_eq!(
        weak.magazine, 3,
        "the weak pointer binds to the second physical Record"
    );
    assert_eq!(
        first.magazine, 2,
        "the control points to the first physical Record"
    );
    assert_eq!(
        weak.speed.to_bits(),
        200.0_f32.to_bits(),
        "the exact weak target supplies ammo"
    );
    assert_eq!(
        first.speed.to_bits(),
        100.0_f32.to_bits(),
        "the first Record supplies its own ammo"
    );
    assert_eq!(
        weak.components, first.components,
        "equal nominal links share a component key"
    );
    assert_ne!(
        weak.key, first.key,
        "the original physical ammo separates the variant keys"
    );
    Ok(())
}

#[test]
fn a_guid_reference_keeps_the_decoder_selected_first_alias() -> TestResult {
    let bytes = fixture(
        TestType::Reference,
        TestValue::Reference(MAGAZINE_GUID),
        TestValue::Reference(MAGAZINE_GUID),
    )
    .build();
    let forge = DataForge::open(&bytes)?;
    let selected = read_item(&forge, &mut TreeBuilder::new(), 0)?;
    assert_eq!(
        selected.magazine, 2,
        "the GUID selects the first physical alias"
    );
    assert_eq!(
        selected.speed.to_bits(),
        100.0_f32.to_bits(),
        "the first alias supplies the stats"
    );
    Ok(())
}

#[test]
fn missing_selected_ammo_keeps_the_legacy_absence_with_complete_source_proof() -> TestResult {
    const MISSING_AMMO: u128 = 41;
    let bytes = fixture_types(TestType::Reference)
        .record(weapon(
            1,
            "FixtureItem",
            TestValue::Reference(MAGAZINE_GUID),
        ))
        .record(magazine(FIRST_MAGAZINE, MISSING_AMMO))
        .build();
    test_support::with_input(&bytes, |_, input| {
        let forge = input.forge();
        let record = forge.record(0).ok_or("physical Item Record")?;
        let mut trees = TreeBuilder::new();
        let tree = trees.build(&record)?;
        let expected = read(&tree, forge, &mut trees).ok_or("legacy Item stats")?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let actual = read_scoped(input, &record, &tree, &fields, &mut trees, None)?;
        let actual_stats = actual.stats.ok_or("source-aware Item stats")?;
        assert_eq!(
            serde_json::to_value(&actual_stats)?,
            serde_json::to_value(&expected)?,
            "source accounting preserves the canonical compatibility value"
        );
        let weapon = actual_stats.weapon.ok_or("the authored weapon component")?;
        assert!(
            weapon.ammo.is_none(),
            "the dangling target supplies no ammunition"
        );
        assert!(
            actual.evidence.origin.source_set_complete(),
            "a proved absent target is distinct from an unreadable selected body"
        );
        let inventory = input.origins().uses.freeze()?;
        assert!(
            inventory.iter_uses().any(|used| {
                matches!(used.location(), p4kx_extraction::SourceLocation::Subresource {
                    namespace, indices, field
                } if namespace == "dataforgeValue" && indices.first() == Some(&1) && field == "$error.guid")
                    && actual.evidence.origin.source_uses_refs().contains(used.id())
            }),
            "the linked magazine's exact missing ammo GUID participates in the stats proof"
        );
        Ok(())
    })
}

struct ReadItem<'source> {
    magazine: u32,
    speed: f32,
    components: ItemComponentKey<'source>,
    key: ItemKey<'source>,
}

fn read_item<'source>(
    forge: &'source DataForge<'source>,
    trees: &mut TreeBuilder<'source>,
    index: u32,
) -> Result<ReadItem<'source>, Box<dyn Error>> {
    let record = forge.record(index).ok_or("physical Item Record")?;
    let tree = trees.build(&record)?;
    let primary =
        component(&tree, WEAPON_COMPONENT).and_then(|weapon| weapon.field(PRIMARY_MAGAZINE_FIELD));
    let Some(Value::Reference(primary)) = primary else {
        return Err("actual decoded magazine target".into());
    };
    let stats = read(&tree, forge, trees).ok_or("legacy Item stats")?;
    let speed = stats
        .weapon
        .as_ref()
        .and_then(|weapon| weapon.ammo.as_ref())
        .and_then(|ammo| ammo.speed)
        .ok_or("reached projectile speed")?;
    let components = item_variant::component_key(&tree).ok_or("matching Item components")?;
    let key = components
        .with_stats(Some(&stats))
        .ok_or("actual complete variant hash")?;
    Ok(ReadItem {
        magazine: primary.original_target().record_index(),
        speed,
        components,
        key,
    })
}

fn fixture(storage: TestType, first: TestValue, second: TestValue) -> DataForgeBuilder {
    fixture_types(storage)
        .record(weapon(1, "FixtureItemWeak", first))
        .record(weapon(2, "FixtureItemFirst", second))
        .record(magazine(FIRST_MAGAZINE, 21))
        .record(magazine(SECOND_MAGAZINE, 22))
        .record(ammo(21, "FixtureAmmoFirst", 100.0))
        .record(ammo(22, "FixtureAmmoSecond", 200.0))
}

fn fixture_types(storage: TestType) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(ENTITY)
                .array("Components", TestType::StrongPointer(COMPONENT.to_owned())),
        )
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(ATTACHMENT)
                .parent(COMPONENT)
                .field("AttachDef", TestType::Class(ATTACH_DEF.to_owned())),
        )
        .structure(
            TestStruct::new(ATTACH_DEF)
                .field("Type", TestType::String)
                .field("Grade", TestType::Int32),
        )
        .structure(
            TestStruct::new(WEAPON_COMPONENT)
                .parent(COMPONENT)
                .field(PRIMARY_MAGAZINE_FIELD, storage),
        )
        .structure(
            TestStruct::new(AMMO_CONTAINER)
                .parent(COMPONENT)
                .field("ammoParamsRecord", TestType::Reference),
        )
        .structure(TestStruct::new(AMMO).field("speed", TestType::Single))
}

fn weapon(guid: u128, name: &str, target: TestValue) -> TestRecord {
    weapon_at(guid, name, format!("fixture/{name}.xml"), target, 1)
}

fn weapon_at(guid: u128, name: &str, path: String, target: TestValue, grade: i32) -> TestRecord {
    let attach = TestInstance::new(ATTACHMENT).value(
        "AttachDef",
        TestValue::Struct(
            TestInstance::new(ATTACH_DEF)
                .value("Type", TestValue::text("Weapon"))
                .value("Grade", TestValue::Int32(grade)),
        ),
    );
    TestRecord::new(guid, ENTITY, name, path).value(
        "Components",
        TestValue::Array(vec![
            TestValue::Struct(attach),
            TestValue::Struct(
                TestInstance::new(WEAPON_COMPONENT).value(PRIMARY_MAGAZINE_FIELD, target),
            ),
        ]),
    )
}

fn magazine(name: &str, ammo: u128) -> TestRecord {
    let container =
        TestInstance::new(AMMO_CONTAINER).value("ammoParamsRecord", TestValue::Reference(ammo));
    TestRecord::new(MAGAZINE_GUID, ENTITY, name, format!("fixture/{name}.xml")).value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(container)]),
    )
}

fn ammo(guid: u128, name: &str, speed: f32) -> TestRecord {
    TestRecord::new(guid, AMMO, name, format!("fixture/{name}.xml"))
        .value("speed", TestValue::Single(speed))
}
