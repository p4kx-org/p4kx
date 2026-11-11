// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, RecordTree, Value};

use super::authored_fields::Observed;
use super::catalog_durability::{self, DurabilityComponent, DurabilityRows, DurabilityTables};
use super::catalog_explosives::{self, Explosive, ExplosiveComponent};
use super::catalog_hacking_chips::{self, HackingChip, HackingComponent};
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

#[derive(Default)]
pub(crate) struct ItemComponentRows<'source> {
    pub(crate) durability: DurabilityRows<'source>,
    pub(crate) explosives: Vec<Explosive<'source>>,
    pub(crate) hacking_chips: Vec<HackingChip<'source>>,
    pub(crate) numbers: Vec<CapturedNumber>,
}

impl<'source> ItemComponentRows<'source> {
    pub(crate) fn extend(&mut self, other: Self) {
        self.durability.extend(other.durability);
        self.explosives.extend(other.explosives);
        self.hacking_chips.extend(other.hacking_chips);
        self.numbers.extend(other.numbers);
    }

    pub(crate) fn into_tables(self, forge: &DataForge<'_>) -> ItemComponentTables<'source> {
        ItemComponentTables {
            durability: catalog_durability::tables(forge, self.durability),
            explosives: catalog_explosives::table(forge, self.explosives),
            hacking_chips: catalog_hacking_chips::table(forge, self.hacking_chips),
        }
    }
}

pub(crate) struct ItemComponentTables<'source> {
    pub(crate) durability: DurabilityTables<'source>,
    pub(crate) explosives: CatalogTable<Explosive<'source>>,
    pub(crate) hacking_chips: CatalogTable<HackingChip<'source>>,
}

#[derive(Default)]
pub(crate) struct ItemComponentsRead<'source> {
    pub(crate) rows: ItemComponentRows<'source>,
    pub(crate) notes: Vec<FieldError>,
    pub(crate) legacy_health: Option<f32>,
}

impl<'source> ItemComponentsRead<'source> {
    fn push(
        &mut self,
        kind: Component,
        owner: ComponentOwner,
        observed: Observed<'_, '_, '_, 'source>,
    ) -> Result<(), CatalogError> {
        match kind {
            Component::Durability(kind) => {
                if let Some(legacy) =
                    self.rows
                        .durability
                        .push(kind, owner, observed, &mut self.notes)?
                {
                    self.legacy_health = Some(legacy);
                }
            }
            Component::Explosive(kind) => {
                self.rows
                    .explosives
                    .push(Explosive::read(kind, owner, observed, &mut self.notes)?);
            }
            Component::Hacking(kind) => {
                self.rows.hacking_chips.push(HackingChip::read(
                    kind,
                    owner,
                    observed,
                    &mut self.notes,
                )?);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Component {
    Durability(DurabilityComponent),
    Explosive(ExplosiveComponent),
    Hacking(HackingComponent),
}

impl Component {
    fn of<'read, 'source>(value: &'read Value<'source>) -> Option<(&'read Value<'source>, Self)> {
        let Value::Struct(source) = value else {
            return None;
        };
        let name = source.type_name();
        let kind = DurabilityComponent::of(name)
            .map(Self::Durability)
            .or_else(|| ExplosiveComponent::of(name).map(Self::Explosive))
            .or_else(|| HackingComponent::of(name).map(Self::Hacking))?;
        Some((value, kind))
    }
}

/// Reads each recognized physical component occurrence using its own admitted Record tree.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ItemComponentsRead<'source>, CatalogError> {
    let mut result = ItemComponentsRead::default();
    let Some(Value::Array(components)) = tree.field("Components") else {
        return Ok(result);
    };
    let mut components = components.iter().filter_map(Component::of).peekable();
    if components.peek().is_none() {
        return Ok(result);
    }
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let context = projection
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let numbers = RefCell::new(Vec::new());

    for (value, kind) in components {
        let component = originals.fact(value)?;
        let owner = ComponentOwner::read(tree, &context, &originals, component)?;
        let observed = Observed::new(&originals, input, component, &capture, &numbers);
        result.push(kind, owner, observed)?;
    }
    result.rows.numbers = numbers.into_inner();
    Ok(result)
}
