// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{CancelToken, DataForge, FieldOrigin, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_table::CatalogTable;
use crate::catalog_v3::ammo_definition::PreparedAmmoDefinition;
use crate::catalog_v3::authored_projectile::{self, AuthoredProjectile};
use crate::catalog_v3::catalog_weapon_projectile_ammo::{
    ProjectileAmmoRole, WeaponProjectileAmmoSelection,
};
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

/// A proven countermeasure ammunition role on an original weapon component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CountermeasureEmitter {
    #[serde(flatten)]
    owner: ComponentOwner,
    route: SourcedValue<EmitterRoute>,
}

/// The original container slot, projectile role and ammunition of an emitter.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct EmitterRoute {
    ammo_role: SourcedValue<ProjectileAmmoRole>,
    container_record_index: SourcedValue<u32>,
    container_component_ordinal: SourcedValue<u32>,
    ammo_record_index: SourcedValue<u32>,
}

impl CountermeasureEmitter {
    /// Returns the original owner and weapon component position.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Returns the proved route and its contributing origins.
    #[must_use]
    pub const fn route(&self) -> &SourcedValue<EmitterRoute> {
        &self.route
    }
}

impl EmitterRoute {
    /// Returns the originally selected ammunition role.
    #[must_use]
    pub const fn ammo_role(&self) -> &SourcedValue<ProjectileAmmoRole> {
        &self.ammo_role
    }

    /// Returns the physical Record owning the container component.
    #[must_use]
    pub const fn container_record_index(&self) -> &SourcedValue<u32> {
        &self.container_record_index
    }

    /// Returns the original container component position within its owner.
    #[must_use]
    pub const fn container_component_ordinal(&self) -> &SourcedValue<u32> {
        &self.container_component_ordinal
    }

    /// Returns the physical Record of the selected ammunition.
    #[must_use]
    pub const fn ammo_record_index(&self) -> &SourcedValue<u32> {
        &self.ammo_record_index
    }
}

struct EmitterEvidence<'rows> {
    owner: &'rows ComponentOwner,
    origins: Vec<&'rows FieldOrigin>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct EmitterKey {
    weapon_record_index: u32,
    weapon_component_ordinal: u32,
    ammo_role: ProjectileAmmoRole,
    container_record_index: u32,
    container_component_ordinal: u32,
    ammo_record_index: u32,
}

impl EmitterKey {
    fn row(
        self,
        evidence: &EmitterEvidence<'_>,
        sources: &SourceUses,
    ) -> Result<CountermeasureEmitter, CatalogError> {
        let origin = sources.value_with_origins(SourceValue::Present(()), &evidence.origins)?;
        let sourced_index = |index| origin.map_ref(|()| index);
        let route = EmitterRoute {
            ammo_role: origin.map_ref(|()| self.ammo_role),
            container_record_index: sourced_index(self.container_record_index),
            container_component_ordinal: sourced_index(self.container_component_ordinal),
            ammo_record_index: sourced_index(self.ammo_record_index),
        };
        Ok(CountermeasureEmitter {
            owner: evidence.owner.clone(),
            route: origin.map(|()| route),
        })
    }
}

pub(super) fn table(
    forge: &DataForge<'_>,
    sources: &SourceUses,
    selections: &[WeaponProjectileAmmoSelection],
    ammunition: &[PreparedAmmoDefinition<'_>],
    cancel: Option<&CancelToken>,
) -> Result<CatalogTable<CountermeasureEmitter>, CatalogError> {
    let rows = rows(sources, selections, ammunition, cancel)?;
    Ok(CatalogTable::new(
        forge,
        &[authored_projectile::COUNTERMEASURE],
        rows,
    ))
}

pub(crate) fn rows(
    sources: &SourceUses,
    selections: &[WeaponProjectileAmmoSelection],
    ammunition: &[PreparedAmmoDefinition<'_>],
    cancel: Option<&CancelToken>,
) -> Result<Vec<CountermeasureEmitter>, CatalogError> {
    let projectiles = countermeasure_origins(ammunition, cancel)?;
    let mut emitters = BTreeMap::<EmitterKey, EmitterEvidence<'_>>::new();
    for selection in selections {
        check_cancel(cancel)?;
        let Some(route) = selection.selected_route() else {
            continue;
        };
        let (Some(&ammo_record_index), Some(&container_component_ordinal), Some(&ammo_role)) = (
            route.ammo_record_index.value(),
            route.container_component_ordinal.value(),
            route.role.value(),
        ) else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let Some(origin) = projectiles.get(&ammo_record_index) else {
            continue;
        };
        let (Some(weapon_record_index), Some(weapon_component_ordinal)) = selection.owner().key()
        else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let container_record_index = route
            .container_record_index
            .map_or(Some(weapon_record_index), |index| index.value().copied())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let key = EmitterKey {
            weapon_record_index,
            weapon_component_ordinal,
            ammo_role,
            container_record_index,
            container_component_ordinal,
            ammo_record_index,
        };
        emitters
            .entry(key)
            .or_insert_with(|| EmitterEvidence {
                owner: selection.owner(),
                origins: vec![origin],
            })
            .origins
            .push(selection.origin());
    }
    emitters
        .into_iter()
        .map(|(key, evidence)| {
            check_cancel(cancel)?;
            key.row(&evidence, sources)
        })
        .collect()
}

fn countermeasure_origins<'rows>(
    ammunition: &'rows [PreparedAmmoDefinition<'_>],
    cancel: Option<&CancelToken>,
) -> Result<BTreeMap<u32, &'rows FieldOrigin>, CatalogError> {
    let mut projectiles = BTreeMap::new();
    for definition in ammunition {
        check_cancel(cancel)?;
        let projectile = definition.inputs().projectile();
        if projectile
            .value()
            .is_some_and(AuthoredProjectile::is_countermeasure)
        {
            projectiles.insert(definition.physical_index, projectile.origin());
        }
    }
    Ok(projectiles)
}

fn check_cancel(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}
