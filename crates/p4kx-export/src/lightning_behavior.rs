// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::lightning_target::{self, LightningTargetMode};
use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const SOURCE_TYPE: &str = "LightningBehavior";
const EFFECT_TYPE: &str = "LightningBehavior_Effect";
const EFFECTS: &str = "effects";
const TARGET_MODES: &str = "targetModes";

type OriginalSlots<T> = SourcedValue<Vec<SourcedValue<T>>>;

/// Original lightning settings with their ordered effect declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LightningBehavior<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    effects: OriginalSlots<LightningEffect<'source>>,
}

impl<'source> LightningBehavior<'source> {
    /// Borrows the concrete type and additional authored fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the original effect slots in source order.
    #[must_use]
    pub const fn effects(&self) -> &SourcedValue<Vec<SourcedValue<LightningEffect<'source>>>> {
        &self.effects
    }

    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let effects = fields.class_array_to(EFFECTS, EFFECT_TYPE, notes, LightningEffect::read)?;
        let source = UnitSource::read(fields, [(EFFECTS, retained_slot_state(&effects))])?;
        Ok(Self { source, effects })
    }
}

/// One authored lightning effect and its ordered target-mode slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LightningEffect<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    target_modes: OriginalSlots<LightningTargetMode<'source>>,
}

impl<'source> LightningEffect<'source> {
    /// Borrows the concrete type and additional authored fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the original target-mode slots in source order.
    #[must_use]
    pub const fn target_modes(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<LightningTargetMode<'source>>>> {
        &self.target_modes
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let target_modes = fields.strong_pointer_family_array_to(
            TARGET_MODES,
            lightning_target::SOURCE_TYPE,
            notes,
            LightningTargetMode::read,
        )?;
        let source =
            UnitSource::read(fields, [(TARGET_MODES, retained_slot_state(&target_modes))])?;
        Ok(Self {
            source,
            target_modes,
        })
    }
}

fn retained_slot_state<T>(array: &OriginalSlots<T>) -> SourceValueState {
    if array.value().is_some_and(|slots| {
        slots.iter().any(|slot| {
            !matches!(
                slot.state(),
                SourceValueState::Present | SourceValueState::Null
            )
        })
    }) {
        return SourceValueState::Unreadable;
    }
    array.state()
}
