// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigin, SourceUseRole, SourceValueState, SourcedValue, Value, ValueError,
};

use super::route;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueEdge, OriginalValuePart};
use crate::catalog_v3::catalog_weapon_actions::WeaponActions;
use crate::{CatalogError, ExportAccountingError};
pub(super) enum ActionSlots {
    Complete(FieldOrigin),
    Repeated(FieldOrigin),
    Incomplete(FieldOrigin),
}

impl ActionSlots {
    pub(super) fn read(
        fields: &OriginalFlowFields<'_, '_, '_>,
        weapon: &WeaponActions<'_>,
        slots: &[SourcedValue<u32>],
    ) -> Result<Self, CatalogError> {
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

    pub(super) const fn origin(&self) -> &FieldOrigin {
        match self {
            Self::Complete(origin) | Self::Repeated(origin) | Self::Incomplete(origin) => origin,
        }
    }
}
