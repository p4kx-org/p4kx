// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForge, DecimalU64, FieldOrigin, SourceUseRole, SourceValue, SourceValueState, SourcedValue,
    Value, ValueError,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::catalog_ammo_bindings::{AmmoBinding, ContainerAmmoSlots};
use super::catalog_weapon_actions::{ProjectileAction, WeaponActions};
use super::catalog_weapon_magazine_slots::WeaponMagazineSlots;
use super::item_component_owner::ComponentOwner;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::ammo_definition;
use crate::definition_reference::DefinitionStatus;
use crate::item_stats::WEAPON_COMPONENT;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

#[path = "weapon_projectile_ammo_route.rs"]
mod route;

use route::OwnRoute;

/// An authored launcher selection tied to one original weapon action occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct WeaponProjectileAmmoSelection {
    #[serde(flatten)]
    owner: ComponentOwner,
    action_source_value_ordinal: SourcedValue<DecimalU64>,
    selection: SourcedValue<Selection>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum Selection {
    Selected {
        role: SourcedValue<ProjectileAmmoRole>,
        container_component_ordinal: SourcedValue<u32>,
        ammo_record_index: SourcedValue<u32>,
    },
    AmbiguousAction,
    IncompleteActions,
    AmbiguousComponents,
    IncompleteComponents,
    NoAdmittedOwnContainer,
    OtherMagazineRoute,
    IndeterminateMagazineRoute,
    UnsupportedProjectileType,
    MissingProjectileType,
    IndeterminateProjectileType,
    MissingAmmo,
    DanglingAmmo,
    UnsupportedAmmoTarget,
    IndeterminateAmmo,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(super) enum ProjectileAmmoRole {
    Primary,
    Secondary,
}

pub(super) fn read<'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    weapons: &[WeaponActions<'source>],
    bindings: &[AmmoBinding<'source>],
) -> Result<Vec<WeaponProjectileAmmoSelection>, CatalogError> {
    if weapons.is_empty() {
        return Ok(Vec::new());
    }
    let route = OwnRoute::read(fields, input, weapons, bindings)?;
    let mut rows = Vec::new();
    for weapon in weapons {
        let Some(slots) = weapon.fire_actions().value() else {
            continue;
        };
        let magazine = magazine_for(weapon, bindings)?;
        let actions = ActionSlots::read(fields, weapon, slots)?;
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
            let mut origins = vec![
                actions.origin(),
                weapon.fire_actions().origin(),
                slot.origin(),
                original.source_ordinal.origin(),
                projectile.launcher_origin,
                projectile.label.origin(),
            ];
            let selection = match &actions {
                ActionSlots::Repeated(_) => Selection::AmbiguousAction,
                ActionSlots::Incomplete(_) => Selection::IncompleteActions,
                ActionSlots::Complete(_) => select(
                    fields,
                    &route,
                    magazine,
                    &original,
                    projectile.label,
                    &mut origins,
                )?,
            };
            rows.push(WeaponProjectileAmmoSelection {
                owner: weapon.owner().clone(),
                action_source_value_ordinal: original.source_ordinal.clone(),
                selection: fields
                    .sources()
                    .value_with_origins(SourceValue::Present(selection), &origins)?,
            });
        }
    }
    Ok(rows)
}

enum ActionSlots {
    Complete(FieldOrigin),
    Repeated(FieldOrigin),
    Incomplete(FieldOrigin),
}

impl ActionSlots {
    fn read(
        fields: &OriginalFlowFields<'_, '_, '_>,
        weapon: &WeaponActions<'_>,
        slots: &[SourcedValue<u32>],
    ) -> Result<Self, CatalogError> {
        use crate::catalog_read::original::OriginalValueEdge;

        let (record, slot) = weapon.owner().key();
        if record != Some(fields.record_index()) {
            return Err(p4kx_extraction::SourceEvidenceError::ForeignEvidence.into());
        }
        let (_, components) = fields
            .field(None, "Components")?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let ordinal = slot.ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let owner = fields.child(
            Some(components.ordinal()),
            OriginalValueEdge::Element {
                ordinal: u64::from(ordinal),
            },
        )?;
        let Value::Struct(source) = owner.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let (_, field) = fields
            .field(Some(owner), "fireActions")?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let Value::Array(original) = field.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        if original.len() != slots.len() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let mut uses = vec![fields.survey(Some(owner))?];
        let mut repeated = false;
        let mut complete = route::at_most_one_declaration(source, "fireActions");
        for (raw, slot) in original.iter().zip(slots) {
            if !matches!(
                slot.state(),
                SourceValueState::Present | SourceValueState::Null
            ) {
                let evidence = fields
                    .fact(raw)?
                    .observe(OriginalValuePart::Value, SourceUseRole::Attempted)?;
                uses.push(evidence);
                complete = false;
                repeated |= matches!(raw, Value::Error(ValueError::OwnedTwice { .. }));
            }
        }
        let origin = fields.sources().origin(&uses, complete);
        if repeated {
            return Ok(Self::Repeated(origin));
        }
        if !complete {
            return Ok(Self::Incomplete(origin));
        }
        Ok(Self::Complete(origin))
    }

    const fn origin(&self) -> &FieldOrigin {
        match self {
            Self::Complete(origin) | Self::Repeated(origin) | Self::Incomplete(origin) => origin,
        }
    }
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
    let selected = match route {
        OwnRoute::Incomplete(_) => return Ok(Selection::IncompleteComponents),
        OwnRoute::Competing(_) => return Ok(Selection::AmbiguousComponents),
        OwnRoute::NoContainer(_) => return Ok(Selection::NoAdmittedOwnContainer),
        OwnRoute::Unique { container, .. } => *container,
    };
    let inputs = magazine.route_inputs();
    origins.extend(inputs.origins());
    let surveys = route::route_surveys(fields, selected, magazine, role)?;
    if !surveys.magazines_unique {
        return Ok(Selection::IndeterminateMagazineRoute);
    }
    if inputs.other_route() {
        return Ok(Selection::OtherMagazineRoute);
    }
    if !inputs.permits_own_route() {
        return Ok(Selection::IndeterminateMagazineRoute);
    }
    if !surveys.ammo_unique {
        return Ok(Selection::IndeterminateAmmo);
    }
    select_ammo(fields, role, selected, selector.origin(), origins)
}

fn select_ammo<'origin, 'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    role: ProjectileAmmoRole,
    container: &'origin ContainerAmmoSlots<'source>,
    selector_origin: &FieldOrigin,
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
    Ok(Selection::Selected {
        role: fields
            .sources()
            .value_with_origins(SourceValue::Present(role), &[selector_origin])?,
        container_component_ordinal: fields
            .sources()
            .value_with_origins(SourceValue::Present(ordinal), origins)?,
        ammo_record_index: fields
            .sources()
            .value_with_origins(SourceValue::Present(ammo_index), origins)?,
    })
}

pub(super) fn table(
    forge: &DataForge<'_>,
    mut rows: Vec<WeaponProjectileAmmoSelection>,
) -> CatalogTable<WeaponProjectileAmmoSelection> {
    rows.sort_by_key(|row| {
        (
            row.owner.key(),
            row.action_source_value_ordinal
                .value()
                .map(|value| value.get()),
        )
    });
    CatalogTable::new(forge, &[WEAPON_COMPONENT], rows)
}
