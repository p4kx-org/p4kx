// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, FieldOrigin, SourceUse, SourceUseRole, StructValue, Value};

use super::{
    AmmoBinding, ContainerAmmoSlots, ProjectileAmmoRole, WeaponActions, WeaponMagazineSlots,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::catalog_v3::catalog_item_components::resolution::SelectionEvidence;
use crate::{CatalogError, ExportAccountingError};

pub(super) enum OwnRoute<'rows, 'source> {
    Unique {
        evidence: FieldOrigin,
        container: &'rows ContainerAmmoSlots<'source>,
    },
    NoContainer(FieldOrigin),
    Competing(FieldOrigin),
    Incomplete(FieldOrigin),
}

impl<'rows, 'source> OwnRoute<'rows, 'source> {
    pub(super) fn read(
        fields: &OriginalFlowFields<'_, '_, 'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        weapons: &[WeaponActions<'source>],
        bindings: &'rows [AmmoBinding<'source>],
    ) -> Result<Self, CatalogError> {
        let mut evidence = SelectionEvidence::default();
        let mut containers = bindings.iter().filter_map(|binding| match binding {
            AmmoBinding::ContainerSlots { value } => Some(value.as_ref()),
            AmmoBinding::WeaponMagazineSlots { .. } => None,
        });
        let container = containers.next();
        if !evidence.retain(fields, None)? {
            return Ok(Self::Incomplete(evidence.into_origin(fields.sources())));
        }
        if weapons.len() != 1 || containers.next().is_some() {
            return Ok(Self::Competing(evidence.into_origin(fields.sources())));
        }
        let Some(container) = container else {
            return Ok(Self::NoContainer(evidence.into_origin(fields.sources())));
        };
        let [weapon] = weapons else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let Some((_, root)) = fields.field(None, "Components")? else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let Value::Array(components) = root.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let storage = root.field().storage();
        evidence.add(fields.survey(None)?);
        if storage.data_type() != DataType::StrongPointer
            || !storage.is_array()
            || !at_most_one_declaration(fields.root(), "Components")
        {
            evidence.attempt(root.observe(OriginalValuePart::DataType, SourceUseRole::Attempted)?);
            return Ok(Self::Incomplete(evidence.into_origin(fields.sources())));
        }

        let (declared, declarations) = root.observe_declared_target(input)?;
        evidence.extend(declarations);
        if declared.is_err() {
            evidence.attempt(root.observe(OriginalValuePart::Target, SourceUseRole::Attempted)?);
            return Ok(Self::Incomplete(evidence.into_origin(fields.sources())));
        }
        let known = [
            original_component(fields, components, weapon.owner().key())?,
            original_component(fields, components, container.owner().key())?,
        ];
        let inventory = component_inventory(
            fields,
            components,
            known,
            u32::from(storage.target()),
            &mut evidence,
        )?;
        let evidence = evidence.into_origin(fields.sources());
        Ok(match inventory {
            ComponentInventory::Unique => Self::Unique {
                evidence,
                container,
            },
            ComponentInventory::Competing => Self::Competing(evidence),
            ComponentInventory::Incomplete => Self::Incomplete(evidence),
        })
    }

    pub(super) const fn origin(&self) -> &FieldOrigin {
        match self {
            Self::Unique { evidence, .. }
            | Self::NoContainer(evidence)
            | Self::Competing(evidence)
            | Self::Incomplete(evidence) => evidence,
        }
    }
}

enum ComponentInventory {
    Unique,
    Competing,
    Incomplete,
}

fn component_inventory(
    fields: &OriginalFlowFields<'_, '_, '_>,
    components: &[Value<'_>],
    known: [(&Value<'_>, u32); 2],
    declared: u32,
    evidence: &mut SelectionEvidence,
) -> Result<ComponentInventory, CatalogError> {
    let families = known.map(|(_, index)| index);
    let mut lineage = Vec::new();
    for component in components {
        let fact = fields.fact(component)?;
        let Value::Struct(_) = component else {
            if !matches!(component, Value::Null) {
                evidence.attempt(fact.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?);
                return Ok(ComponentInventory::Incomplete);
            }
            continue;
        };
        let admitted = has_ancestry(fields, fact, &[declared], &mut lineage, evidence)?;
        if !admitted {
            evidence
                .attempt(fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Attempted)?);
            return Ok(ComponentInventory::Incomplete);
        }
        if known
            .iter()
            .any(|(value, _)| std::ptr::eq(*value, component))
        {
            continue;
        }
        if has_ancestry(fields, fact, &families, &mut lineage, evidence)? {
            return Ok(ComponentInventory::Competing);
        }
    }
    Ok(ComponentInventory::Unique)
}

fn has_ancestry(
    fields: &OriginalFlowFields<'_, '_, '_>,
    component: &OriginalValueFact<'_, '_, '_>,
    candidates: &[u32],
    lineage: &mut Vec<SourceUse>,
    evidence: &mut SelectionEvidence,
) -> Result<bool, CatalogError> {
    let matches = component.descends_from_any(candidates, lineage, || fields.check_cancel())?;
    evidence.append(lineage);
    Ok(matches)
}

pub(super) fn at_most_one_declaration(structure: &StructValue<'_>, name: &str) -> bool {
    structure
        .fields()
        .iter()
        .filter(|field| field.name() == name)
        .nth(1)
        .is_none()
}

pub(super) fn selector_is_unique<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    action: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<bool, CatalogError> {
    let Value::Struct(source) = action.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let Some((_, launcher)) = fields.field(Some(action), "launchParams")? else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let Value::Struct(launcher) = launcher.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    Ok(at_most_one_declaration(source, "launchParams")
        && at_most_one_declaration(launcher, "projectileType"))
}

pub(super) struct RouteSurveys {
    pub(super) magazines_unique: bool,
    pub(super) ammo_unique: bool,
}

pub(super) fn route_surveys(
    fields: &OriginalFlowFields<'_, '_, '_>,
    container: &ContainerAmmoSlots<'_>,
    magazine: &WeaponMagazineSlots<'_>,
    role: ProjectileAmmoRole,
) -> Result<RouteSurveys, CatalogError> {
    let Some((_, root)) = fields.field(None, "Components")? else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let Value::Array(components) = root.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let (Value::Struct(weapon), _) =
        original_component(fields, components, magazine.owner().key())?
    else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let (Value::Struct(container), _) =
        original_component(fields, components, container.owner().key())?
    else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let ammo_field = match role {
        ProjectileAmmoRole::Primary => "ammoParamsRecord",
        ProjectileAmmoRole::Secondary => "secondaryAmmoParamsRecord",
    };
    Ok(RouteSurveys {
        magazines_unique: [
            crate::item_stats::PRIMARY_MAGAZINE_FIELD,
            "secondaryAmmoContainers",
            "ShouldIgnorePrimaryAmmoContainer",
        ]
        .into_iter()
        .all(|name| at_most_one_declaration(weapon, name)),
        ammo_unique: at_most_one_declaration(container, ammo_field),
    })
}

fn original_component<'borrow, 'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    components: &'borrow [Value<'source>],
    (record, slot): (Option<u32>, Option<u32>),
) -> Result<(&'borrow Value<'source>, u32), CatalogError> {
    if record != Some(fields.record_index()) {
        return Err(p4kx_extraction::SourceEvidenceError::ForeignEvidence.into());
    }
    let index = slot
        .and_then(|slot| usize::try_from(slot).ok())
        .ok_or(ExportAccountingError::CounterOverflow)?;
    let Some(component @ Value::Struct(original)) = components.get(index) else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    Ok((component, original.struct_index()))
}
