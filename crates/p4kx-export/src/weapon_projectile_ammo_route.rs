// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataType, FieldOrigin, SourceEvidenceError, SourceUseRole, StructValue, Value,
};

use super::{
    AmmoBinding, ContainerAmmoSlots, ProjectileAmmoRole, WeaponActions, WeaponMagazineSlots,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::catalog_v3::catalog_item_components::resolution::SelectionEvidence;
use crate::item_stats::{AMMO_CONTAINER, WEAPON_COMPONENT};
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
        let Some(components) = surveyed_components(fields, input, &mut evidence)? else {
            return Ok(Self::Incomplete(evidence.into_origin(fields.sources())));
        };
        let mut containers = bindings.iter().filter_map(|binding| match binding {
            AmmoBinding::ContainerSlots { value } => Some(value.as_ref()),
            AmmoBinding::WeaponMagazineSlots { .. } => None,
        });
        let container = containers.next();
        let [weapon] = weapons else {
            return Ok(Self::Competing(evidence.into_origin(fields.sources())));
        };
        if containers.next().is_some() {
            return Ok(Self::Competing(evidence.into_origin(fields.sources())));
        }
        let known_weapon = original_component(fields, components.values, weapon.owner().key())?;
        let own = container
            .map(|known| original_component(fields, components.values, known.owner().key()))
            .transpose()?;
        let inventory = components.inventory(
            fields,
            input,
            &[Some(known_weapon.0), own.map(|pair| pair.0)],
            &mut evidence,
        )?;
        let evidence = evidence.into_origin(fields.sources());
        Ok(match (inventory, container) {
            (ComponentInventory::Unique, Some(container)) => Self::Unique {
                evidence,
                container,
            },
            (ComponentInventory::Unique, None) => Self::NoContainer(evidence),
            (ComponentInventory::Competing, _) => Self::Competing(evidence),
            (ComponentInventory::Incomplete, _) => Self::Incomplete(evidence),
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

pub(super) enum LinkedInventory<'fact, 'scope, 'read, 'source> {
    Unique {
        container: &'fact OriginalValueFact<'scope, 'read, 'source>,
        evidence: FieldOrigin,
    },
    NoContainer(FieldOrigin),
    Competing(FieldOrigin),
    Incomplete(FieldOrigin),
}

pub(super) fn linked_container<'fact, 'scope, 'read, 'source>(
    fields: &'fact OriginalFlowFields<'scope, 'read, 'source>,
    input: &OriginalCatalogInput<'_, 'source>,
) -> Result<LinkedInventory<'fact, 'scope, 'read, 'source>, CatalogError> {
    let mut evidence = SelectionEvidence::default();
    let Some(components) = surveyed_components(fields, input, &mut evidence)? else {
        return Ok(LinkedInventory::Incomplete(
            evidence.into_origin(fields.sources()),
        ));
    };
    let mut explicit = components.values.iter().filter(|value| {
        matches!(value, Value::Struct(structure) if structure.type_name() == AMMO_CONTAINER)
    });
    let actual = explicit.next();
    if explicit.next().is_some() {
        return Ok(LinkedInventory::Competing(
            evidence.into_origin(fields.sources()),
        ));
    }
    let inventory = components.inventory(fields, input, &[actual], &mut evidence)?;
    let evidence = evidence.into_origin(fields.sources());
    Ok(match (inventory, actual) {
        (ComponentInventory::Unique, Some(container)) => LinkedInventory::Unique {
            container: fields.fact(container)?,
            evidence,
        },
        (ComponentInventory::Unique, None) => LinkedInventory::NoContainer(evidence),
        (ComponentInventory::Competing, _) => LinkedInventory::Competing(evidence),
        (ComponentInventory::Incomplete, _) => LinkedInventory::Incomplete(evidence),
    })
}

enum ComponentInventory {
    Unique,
    Competing,
    Incomplete,
}

struct ComponentArray<'read, 'source> {
    values: &'read [Value<'source>],
    declared: Option<u32>,
}

impl ComponentArray<'_, '_> {
    fn inventory(
        &self,
        fields: &OriginalFlowFields<'_, '_, '_>,
        input: &OriginalCatalogInput<'_, '_>,
        known: &[Option<&Value<'_>>],
        evidence: &mut SelectionEvidence,
    ) -> Result<ComponentInventory, CatalogError> {
        let mut lineage = Vec::new();
        let mut competing = false;
        for component in self.values {
            let fact = fields.fact(component)?;
            let Value::Struct(_) = component else {
                if !matches!(component, Value::Null) {
                    evidence
                        .attempt(fact.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?);
                    return Ok(ComponentInventory::Incomplete);
                }
                continue;
            };
            if let Some(declared) = self.declared
                && !fact.descends_from_any(&[declared], &mut lineage, || fields.check_cancel())?
            {
                evidence.append(&mut lineage);
                evidence.attempt(
                    fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Attempted)?,
                );
                return Ok(ComponentInventory::Incomplete);
            }
            evidence.append(&mut lineage);
            if known
                .iter()
                .flatten()
                .any(|known| std::ptr::eq(*known, component))
            {
                continue;
            }
            let family = fact.descends_from_any_named(
                input,
                &[WEAPON_COMPONENT, AMMO_CONTAINER],
                &mut lineage,
                || fields.check_cancel(),
            )?;
            evidence.append(&mut lineage);
            let Some(matches) = family else {
                evidence.attempt(
                    fact.observe(OriginalValuePart::ConcreteType, SourceUseRole::Attempted)?,
                );
                return Ok(ComponentInventory::Incomplete);
            };
            competing |= matches;
        }
        Ok(if competing {
            ComponentInventory::Competing
        } else {
            ComponentInventory::Unique
        })
    }
}

fn surveyed_components<'read, 'source>(
    fields: &OriginalFlowFields<'_, 'read, 'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    evidence: &mut SelectionEvidence,
) -> Result<Option<ComponentArray<'read, 'source>>, CatalogError> {
    evidence.add(fields.survey(None)?);
    if !evidence.retain(fields, None)? {
        return Ok(None);
    }
    let Some((_, root)) = fields.field(None, "Components")? else {
        return Ok(Some(ComponentArray {
            values: &[],
            declared: None,
        }));
    };
    let storage = root.field().storage();
    if storage.data_type() != DataType::StrongPointer
        || !storage.is_array()
        || !at_most_one_declaration(fields.root(), "Components")
    {
        evidence.attempt(root.observe(OriginalValuePart::DataType, SourceUseRole::Attempted)?);
        return Ok(None);
    }
    let (declared, declarations) = root.observe_declared_target(input)?;
    evidence.extend(declarations);
    if declared.is_err() {
        evidence.attempt(root.observe(OriginalValuePart::Target, SourceUseRole::Attempted)?);
        return Ok(None);
    }
    let values = match root.value() {
        Value::Array(values) => values.as_ref(),
        Value::Null => &[],
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    };
    Ok(Some(ComponentArray {
        values,
        declared: Some(u32::from(storage.target())),
    }))
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

pub(super) fn magazine_route_fields_unique(
    fields: &OriginalFlowFields<'_, '_, '_>,
    magazine: &WeaponMagazineSlots<'_>,
) -> Result<bool, CatalogError> {
    let components = components(fields)?;
    let (_, weapon) = original_component(fields, components, magazine.owner().key())?;
    Ok([
        crate::item_stats::PRIMARY_MAGAZINE_FIELD,
        "secondaryAmmoContainers",
        "ShouldIgnorePrimaryAmmoContainer",
    ]
    .into_iter()
    .all(|name| at_most_one_declaration(weapon, name)))
}

pub(super) fn ammo_slot_is_unique(
    fields: &OriginalFlowFields<'_, '_, '_>,
    container: &ContainerAmmoSlots<'_>,
    role: ProjectileAmmoRole,
) -> Result<bool, CatalogError> {
    let components = components(fields)?;
    let (_, original) = original_component(fields, components, container.owner().key())?;
    Ok(at_most_one_declaration(original, role.field_name()))
}

pub(super) fn components<'read, 'source>(
    fields: &OriginalFlowFields<'_, 'read, 'source>,
) -> Result<&'read [Value<'source>], CatalogError> {
    let Some((_, root)) = fields.field(None, "Components")? else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    let Value::Array(components) = root.value() else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    Ok(components)
}

pub(super) fn original_component<'borrow, 'source>(
    fields: &OriginalFlowFields<'_, '_, 'source>,
    components: &'borrow [Value<'source>],
    (record, slot): (Option<u32>, Option<u32>),
) -> Result<(&'borrow Value<'source>, &'borrow StructValue<'source>), CatalogError> {
    if record != Some(fields.record_index()) {
        return Err(SourceEvidenceError::ForeignEvidence.into());
    }
    let index = slot
        .and_then(|slot| usize::try_from(slot).ok())
        .ok_or(ExportAccountingError::CounterOverflow)?;
    let Some(component @ Value::Struct(original)) = components.get(index) else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    Ok((component, original))
}
