// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, FieldOrigin, SourceValue, SourceValueState, SourcedValue};

use super::catalog_ammo_bindings::{AmmoBinding, ContainerAmmoSlots};
use super::catalog_weapon_actions::{ProjectileAction, ProjectileSelector, WeaponActions};
use super::catalog_weapon_magazine_slots::WeaponMagazineSlots;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::ammo_definition;
use crate::definition_reference::DefinitionStatus;
use crate::export_ledger::ExportLedger;
use crate::item_stats::{MagazineRead, WEAPON_COMPONENT};
use crate::{CatalogError, ExportAccountingError};

#[path = "weapon_projectile_action_slots.rs"]
mod action_slots;
#[path = "weapon_linked_magazine.rs"]
mod linked_reader;
#[path = "weapon_magazine_choice.rs"]
mod linked_selection;
#[path = "weapon_projectile_ammo_route.rs"]
mod route;
#[path = "weapon_projectile_ammo_selection.rs"]
mod selection;

use action_slots::ActionSlots;
pub(crate) use linked_reader::LinkedAmmunitionReader;
use linked_reader::LinkedMagazineRead;
use linked_selection::MagazineChoice;
use route::OwnRoute;

pub(in crate::catalog_v3) use selection::ProjectileAmmoRole;
use selection::Selection;
pub(crate) use selection::WeaponProjectileAmmoSelection;

pub(crate) fn read<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    reader: &LinkedAmmunitionReader<'_, 'source>,
    weapons: &[WeaponActions<'source>],
    bindings: &[AmmoBinding<'source>],
    observed: Option<MagazineRead<'_, 'source>>,
    ledger: &mut ExportLedger,
) -> Result<Vec<WeaponProjectileAmmoSelection>, CatalogError> {
    if weapons.is_empty() {
        return Ok(Vec::new());
    }
    let route = OwnRoute::read(fields, reader.projection.original(), weapons, bindings)?;
    let linked = observed.and_then(|observed| LinkedMagazineRead::capture(observed.into_parts().1));
    let mut rows = Vec::new();
    for weapon in weapons {
        let Some(slots) = weapon.fire_actions().value() else {
            continue;
        };
        let magazine = magazine_for(weapon, bindings)?;
        let actions = ActionSlots::read(fields, weapon, slots)?;
        let mut choice = None;
        for slot in slots {
            fields.check_cancel()?;
            let Some(&index) = slot.value() else {
                continue;
            };
            let original = weapon
                .action(index)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            let Some(ref projectile) = original.selector else {
                continue;
            };
            let mut origins = projectile_origins(&actions, weapon, slot, &original, projectile);
            let selection = match &actions {
                ActionSlots::Repeated(_) => Selection::AmbiguousAction,
                ActionSlots::Incomplete(_) => Selection::IncompleteActions,
                ActionSlots::Complete(_) => {
                    let choice = magazine_choice(
                        &mut choice,
                        reader,
                        fields,
                        &route,
                        magazine,
                        linked,
                        ledger,
                    )?;
                    select(
                        fields,
                        &route,
                        magazine,
                        choice,
                        &original,
                        projectile.label,
                        &mut origins,
                    )?
                }
            };
            rows.push(WeaponProjectileAmmoSelection::new(
                weapon.owner().clone(),
                original.source_ordinal.clone(),
                fields
                    .sources()
                    .value_with_origins(SourceValue::Present(selection), &origins)?,
            ));
        }
    }
    Ok(rows)
}

fn projectile_origins<'origin>(
    actions: &'origin ActionSlots,
    weapon: &'origin WeaponActions<'_>,
    slot: &'origin SourcedValue<u32>,
    action: &ProjectileAction<'origin, '_>,
    projectile: &ProjectileSelector<'origin, '_>,
) -> Vec<&'origin FieldOrigin> {
    vec![
        actions.origin(),
        weapon.fire_actions().origin(),
        slot.origin(),
        action.source_ordinal.origin(),
        projectile.launcher_origin,
        projectile.label.origin(),
    ]
}

fn magazine_choice<'choice, 'source>(
    choice: &'choice mut Option<MagazineChoice<'source>>,
    reader: &LinkedAmmunitionReader<'_, 'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    route: &OwnRoute<'_, 'source>,
    magazine: &WeaponMagazineSlots<'source>,
    linked: Option<LinkedMagazineRead<'_, 'source>>,
    ledger: &mut ExportLedger,
) -> Result<&'choice MagazineChoice<'source>, CatalogError> {
    if choice.is_none() {
        *choice = Some(MagazineChoice::read(
            reader, fields, route, magazine, linked, ledger,
        )?);
    }
    choice
        .as_ref()
        .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
}

fn magazine_for<'rows, 'source>(
    weapon: &WeaponActions<'source>,
    bindings: &'rows [AmmoBinding<'source>],
) -> Result<&'rows WeaponMagazineSlots<'source>, CatalogError> {
    bindings
        .iter()
        .find_map(|binding| match binding {
            AmmoBinding::WeaponMagazineSlots { value }
                if value.owner().key() == weapon.owner().key() =>
            {
                Some(value.as_ref())
            }
            _ => None,
        })
        .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
}

fn select<'origin, 'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    route: &'origin OwnRoute<'_, 'source>,
    magazine: &'origin WeaponMagazineSlots<'source>,
    choice: &'origin MagazineChoice<'source>,
    action: &ProjectileAction<'origin, 'source>,
    selector: &'origin SourcedValue<&'source str>,
    origins: &mut Vec<&'origin FieldOrigin>,
) -> Result<Selection, CatalogError> {
    let Some(&ordinal) = action.source_ordinal.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let index =
        usize::try_from(ordinal.get()).map_err(|_| ExportAccountingError::CounterOverflow)?;
    let original = fields
        .facts()
        .get(index)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    if !route::selector_is_unique(fields, original)? {
        return Ok(Selection::IndeterminateProjectileType);
    }
    let role = match selector.value().copied() {
        Some("Primary") => ProjectileAmmoRole::Primary,
        Some("Secondary") => ProjectileAmmoRole::Secondary,
        Some(_) => return Ok(Selection::UnsupportedProjectileType),
        None if matches!(
            selector.state(),
            SourceValueState::Absent | SourceValueState::Null
        ) =>
        {
            return Ok(Selection::MissingProjectileType);
        }
        None => return Ok(Selection::IndeterminateProjectileType),
    };
    origins.push(route.origin());
    match route {
        OwnRoute::Incomplete(_) => return Ok(Selection::IncompleteComponents),
        OwnRoute::Competing(_) => return Ok(Selection::AmbiguousComponents),
        OwnRoute::NoContainer(_) | OwnRoute::Unique { .. } => {}
    }
    let inputs = magazine.route_inputs();
    origins.extend(inputs.origins());
    if let Some(reference) = inputs.primary().value() {
        origins.extend([
            reference.target_type.origin(),
            reference.target_record_index.origin(),
        ]);
    }
    match choice.selected(role, origins) {
        Err(status) => Ok(status),
        Ok(Some(container)) => {
            select_ammo(fields, role, container, selector.origin(), true, origins)
        }
        Ok(None) => {
            let OwnRoute::Unique { container, .. } = route else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            if !route::ammo_slot_is_unique(fields, container, role)? {
                return Ok(Selection::IndeterminateAmmo);
            }
            select_ammo(fields, role, container, selector.origin(), false, origins)
        }
    }
}

fn select_ammo<'origin, 'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    role: ProjectileAmmoRole,
    container: &'origin ContainerAmmoSlots<'source>,
    selector_origin: &FieldOrigin,
    linked: bool,
    origins: &mut Vec<&'origin FieldOrigin>,
) -> Result<Selection, CatalogError> {
    let ammo = container.slot(role);
    origins.push(ammo.origin());
    let Some(reference) = ammo.value() else {
        return Ok(match ammo.state() {
            SourceValueState::Absent | SourceValueState::Null => Selection::MissingAmmo,
            _ => Selection::IndeterminateAmmo,
        });
    };
    match reference.status {
        DefinitionStatus::Dangling => return Ok(Selection::DanglingAmmo),
        DefinitionStatus::UnexpectedTargetType => {
            origins.push(reference.target_type.origin());
            return Ok(Selection::UnsupportedAmmoTarget);
        }
        DefinitionStatus::Present => {}
    }
    let target = &reference.target_record_index;
    origins.extend([target.origin(), reference.target_type.origin()]);
    match reference.target_type.value().copied() {
        Some(ammo_definition::SOURCE_TYPE) => {}
        Some(_) => return Ok(Selection::UnsupportedAmmoTarget),
        None => return Ok(Selection::IndeterminateAmmo),
    }
    let Some(&ordinal) = container.owner().component_ordinal().value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let Some(&ammo_index) = target.value() else {
        return Ok(Selection::IndeterminateAmmo);
    };
    origins.push(container.owner().component_ordinal().origin());
    let sources = fields.sources();
    let role = sources.value_with_origins(SourceValue::Present(role), &[selector_origin])?;
    let container_component_ordinal =
        sources.value_with_origins(SourceValue::Present(ordinal), origins)?;
    let ammo_record_index =
        sources.value_with_origins(SourceValue::Present(ammo_index), origins)?;
    if linked {
        let magazine_index = container
            .owner()
            .key()
            .0
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let magazine_record_index =
            sources.value_with_origins(SourceValue::Present(magazine_index), origins)?;
        return Ok(Selection::SelectedLinkedMagazine {
            role,
            magazine_record_index,
            container_component_ordinal,
            ammo_record_index,
        });
    }
    Ok(Selection::Selected {
        role,
        container_component_ordinal,
        ammo_record_index,
    })
}

pub(super) fn table(
    forge: &DataForge<'_>,
    mut rows: Vec<WeaponProjectileAmmoSelection>,
) -> CatalogTable<WeaponProjectileAmmoSelection> {
    rows.sort_by_key(|row| {
        (
            row.owner().key(),
            row.action_source_value_ordinal()
                .value()
                .map(|value| value.get()),
        )
    });
    CatalogTable::new(forge, &[WEAPON_COMPONENT], rows)
}
