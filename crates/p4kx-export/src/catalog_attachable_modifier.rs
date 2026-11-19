// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use super::attachable_effect::AttachableEffect;
use crate::CatalogError;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const COMPONENT: &str = "EntityComponentAttachableModifierParams";

/// One original attachable modifier component and its ordered effect slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct AttachableModifier<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    activation_method: SourcedValue<&'source str>,
    charges: SourcedValue<i32>,
    can_interrupt: SourcedValue<bool>,
    is_interruptible: SourcedValue<bool>,
    modifiers: SourcedValue<Vec<SourcedValue<AttachableEffect<'source>>>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> AttachableModifier<'source> {
    /// Borrows the original physical component owner.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the original activation-method enum label.
    #[must_use]
    pub const fn activation_method(&self) -> &SourcedValue<&'source str> {
        &self.activation_method
    }

    /// Borrows the original charge count.
    #[must_use]
    pub const fn charges(&self) -> &SourcedValue<i32> {
        &self.charges
    }

    /// Borrows the flag that allows interrupting other modifiers.
    #[must_use]
    pub const fn can_interrupt(&self) -> &SourcedValue<bool> {
        &self.can_interrupt
    }

    /// Borrows the flag that allows this modifier to be interrupted.
    #[must_use]
    pub const fn is_interruptible(&self) -> &SourcedValue<bool> {
        &self.is_interruptible
    }

    /// Borrows the authored modifier slots in original order.
    #[must_use]
    pub const fn modifiers(&self) -> &SourcedValue<Vec<SourcedValue<AttachableEffect<'source>>>> {
        &self.modifiers
    }

    /// Borrows the original type and remaining fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.original
    }

    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let activation_method = source.borrowed_declared_enum("activationMethod", notes)?;
        let charges = source.int32("charges", notes)?;
        let can_interrupt = source.boolean("canInterrupt", notes)?;
        let is_interruptible = source.boolean("isInterruptible", notes)?;
        let modifiers = source.declared_family_array("modifiers", notes, AttachableEffect::read)?;
        let original = UnitSource::read(
            source,
            [
                ("activationMethod", activation_method.state()),
                ("charges", charges.state()),
                ("canInterrupt", can_interrupt.state()),
                ("isInterruptible", is_interruptible.state()),
                ("modifiers", modifiers.state()),
            ],
        )?;
        Ok(Self {
            owner,
            activation_method,
            charges,
            can_interrupt,
            is_interruptible,
            modifiers,
            original,
        })
    }
}
