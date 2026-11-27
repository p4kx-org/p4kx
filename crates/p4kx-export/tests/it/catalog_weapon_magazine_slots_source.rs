// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, Field, RecordTree, StructValue, Value, ValueError};

use super::fixture;
use crate::commodity_v3_fixture::{self as common, TestResult};

pub(super) const DIRECT_INDEX: u32 = 2;
pub(super) const PERSONAL_INDEX: u32 = 3;
pub(super) const PRIMARY_INDEX: u32 = 4;
const SECONDARY_INDEX: u32 = 5;
const AMMO_INDEX: u32 = 1;

pub(super) struct Weapon<'read, 'source> {
    pub(super) component: &'read Value<'source>,
    pub(super) primary: &'read Value<'source>,
    pub(super) secondary: Option<&'read [Value<'source>]>,
    pub(super) ignore_primary: Option<&'read Value<'source>>,
}

pub(super) struct Slot<'read, 'source> {
    pub(super) magazine: &'read Value<'source>,
    pub(super) condition: &'read Value<'source>,
}

pub(super) fn tree<'source>(
    forge: &'source DataForge<'_>,
    index: u32,
) -> TestResult<RecordTree<'source>> {
    let record = forge
        .record(index)
        .ok_or("original physical weapon Record")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the tree is from the selected original Record"
    );
    Ok(tree)
}

pub(super) fn weapon<'read, 'source>(
    forge: &DataForge<'source>,
    tree: &'read RecordTree<'source>,
) -> TestResult<Weapon<'read, 'source>> {
    let Some(Value::Array(components)) = tree.field("Components") else {
        return Err("original Item Components".into());
    };
    let component = components.get(1).ok_or("original weapon component slot")?;
    let weapon = structure(component)?;
    assert_eq!(
        weapon.type_name(),
        "SCItemWeaponComponentParams",
        "slot 1 is the actual selected weapon component"
    );
    if tree.record_index() == DIRECT_INDEX {
        assert_eq!(
            structure(components.get(2).ok_or("original direct container")?)?.type_name(),
            "SAmmoContainerComponentParams",
            "the direct weapon also has an actual own primary container component"
        );
    }
    let primary = declaration(
        forge,
        weapon,
        "ammoContainerRecord",
        DataType::Reference,
        false,
        "EntityClassDefinition",
    )?;
    let secondary = if weapon.field("secondaryAmmoContainers").is_some() {
        match declaration(
            forge,
            weapon,
            "secondaryAmmoContainers",
            DataType::Class,
            true,
            "SSecondaryAmmoContainerParams",
        )? {
            Value::Array(values) => Some(values.as_ref()),
            _ => return Err("actual secondary magazine array".into()),
        }
    } else {
        None
    };
    Ok(Weapon {
        component,
        primary,
        secondary,
        ignore_primary: weapon.field("ShouldIgnorePrimaryAmmoContainer"),
    })
}

pub(super) fn primary(value: &Value<'_>) -> TestResult {
    target(
        value,
        PRIMARY_INDEX,
        fixture::PRIMARY_MAGAZINE,
        "EntityClassDefinition",
    )
}

pub(super) fn slots<'read, 'source>(
    forge: &DataForge<'source>,
    weapon: &Weapon<'read, 'source>,
    case: fixture::MagazineCase,
) -> TestResult<[Slot<'read, 'source>; 3]> {
    let [first, second, third] = weapon.secondary.ok_or("stored secondary array")? else {
        return Err("three actual ordered secondary slots".into());
    };
    let first = slot(forge, first)?;
    let second = slot(forge, second)?;
    let third = slot(forge, third)?;
    match case {
        fixture::MagazineCase::Resolved => {
            target(
                first.magazine,
                SECONDARY_INDEX,
                fixture::SECONDARY_MAGAZINE,
                "EntityClassDefinition",
            )?;
            target(
                second.magazine,
                SECONDARY_INDEX,
                fixture::SECONDARY_MAGAZINE,
                "EntityClassDefinition",
            )?;
            primary(third.magazine)?;
        }
        fixture::MagazineCase::BadLinks => {
            let Value::Error(ValueError::DanglingReference { guid }) = first.magazine else {
                return Err("actual dangling reference in first secondary slot".into());
            };
            assert_eq!(
                guid.to_string(),
                common::guid(fixture::MISSING_MAGAZINE),
                "first slot retains its stored GUID"
            );
            target(second.magazine, AMMO_INDEX, fixture::TACHYON, "AmmoParams")?;
            assert!(
                matches!(third.magazine, Value::Null),
                "last magazine is stored null"
            );
        }
    }
    conditions([&first, &second, &third])?;
    Ok([first, second, third])
}

fn conditions([first, second, third]: [&Slot<'_, '_>; 3]) -> TestResult {
    let fire = structure(first.condition)?;
    assert_eq!(
        fire.type_name(),
        "SWeaponConditionTargetingFire",
        "first slot's actual concrete condition"
    );
    assert!(
        matches!(fire.field("comparer"), Some(Value::Bool(false))),
        "the actual false condition comparator"
    );
    let held = structure(second.condition)?;
    assert_eq!(
        held.type_name(),
        "SWeaponConditionIsHeldEntity",
        "second slot's distinct concrete condition"
    );
    assert!(
        held.fields().is_empty(),
        "the actual condition has no fields"
    );
    assert!(
        matches!(third.condition, Value::Null),
        "last condition is stored null"
    );
    Ok(())
}

fn slot<'read, 'source>(
    forge: &DataForge<'source>,
    value: &'read Value<'source>,
) -> TestResult<Slot<'read, 'source>> {
    let slot = structure(value)?;
    assert_eq!(
        slot.type_name(),
        "SSecondaryAmmoContainerParams",
        "actual secondary Class element"
    );
    Ok(Slot {
        magazine: declaration(
            forge,
            slot,
            "ammoContainerRecord",
            DataType::Reference,
            false,
            "EntityClassDefinition",
        )?,
        condition: declaration(
            forge,
            slot,
            "condition",
            DataType::StrongPointer,
            false,
            "SWeaponConditionBase",
        )?,
    })
}

fn target(raw: &Value<'_>, index: u32, guid: u128, target_type: &str) -> TestResult {
    let Value::Reference(reference) = raw else {
        return Err("actual resolved original Reference".into());
    };
    assert_eq!(
        reference.record_index(),
        index,
        "the actual selected physical Record"
    );
    assert_eq!(
        reference.guid().to_string(),
        common::guid(guid),
        "the actual stored GUID"
    );
    assert_eq!(
        reference.type_name(),
        target_type,
        "the actual selected Record type"
    );
    Ok(())
}

fn declaration<'read, 'source>(
    forge: &DataForge<'source>,
    parent: &'read StructValue<'source>,
    name: &str,
    kind: DataType,
    array: bool,
    target: &str,
) -> TestResult<&'read Value<'source>> {
    let field = parent
        .fields()
        .iter()
        .find(|field| field.name() == name)
        .ok_or("actual original field declaration")?;
    let storage = field.storage();
    assert_eq!(
        storage.data_type(),
        kind,
        "{name} has the original storage type"
    );
    assert_eq!(
        storage.is_array(),
        array,
        "{name} has the original array shape"
    );
    let declared = forge
        .structs()
        .nth(usize::from(storage.target()))
        .ok_or("original declared target Struct")?;
    assert_eq!(
        declared.name()?,
        target,
        "{name} has the original nominal target"
    );
    Ok(field.value())
}

pub(super) fn structure<'read, 'source>(
    value: &'read Value<'source>,
) -> TestResult<&'read StructValue<'source>> {
    let Value::Struct(owner) = value else {
        return Err("actual owned Struct".into());
    };
    Ok(owner)
}

pub(super) fn ordinal<'source>(
    tree: &RecordTree<'source>,
    selected: &Value<'source>,
) -> TestResult<u64> {
    let mut pending: Vec<_> = tree.fields().iter().rev().map(Field::value).collect();
    let mut ordinal = 0_u64;
    while let Some(value) = pending.pop() {
        if std::ptr::eq(value, selected) {
            return Ok(ordinal);
        }
        ordinal = ordinal
            .checked_add(1)
            .ok_or("original occurrence overflow")?;
        match value {
            Value::Struct(owner) => {
                pending.extend(owner.fields().iter().rev().map(Field::value));
            }
            Value::Array(values) => pending.extend(values.iter().rev()),
            _ => {}
        }
    }
    Err("the selected value does not belong to the actual Record tree".into())
}
