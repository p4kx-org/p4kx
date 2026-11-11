// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, RecordTree, Value};

use super::authored_fields::Observed;
use super::catalog_durability::{DurabilityComponent, DurabilityRows};
use super::catalog_explosives::{Explosive, ExplosiveComponent};
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;

#[derive(Default)]
pub(crate) struct ItemComponentRows<'source> {
    pub(crate) durability: DurabilityRows<'source>,
    pub(crate) explosives: Vec<Explosive<'source>>,
    pub(crate) numbers: Vec<CapturedNumber>,
}

impl ItemComponentRows<'_> {
    pub(crate) fn extend(&mut self, other: Self) {
        self.durability.extend(other.durability);
        self.explosives.extend(other.explosives);
        self.numbers.extend(other.numbers);
    }
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
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Component {
    Durability(DurabilityComponent),
    Explosive(ExplosiveComponent),
}

impl Component {
    fn of<'read, 'source>(value: &'read Value<'source>) -> Option<(&'read Value<'source>, Self)> {
        let Value::Struct(source) = value else {
            return None;
        };
        let name = source.type_name();
        let kind = DurabilityComponent::of(name)
            .map(Self::Durability)
            .or_else(|| ExplosiveComponent::of(name).map(Self::Explosive))?;
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
        let observed = Observed::new(&originals, component, &capture, &numbers);
        result.push(kind, owner, observed)?;
    }
    result.rows.numbers = numbers.into_inner();
    Ok(result)
}
