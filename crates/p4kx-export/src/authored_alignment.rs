// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUseRole, SourceValue, SourcedValue};

use super::Observed;
use crate::catalog_read::original::OriginalValuePart;
use crate::catalog_v3::item_alignment_options::{
    AlignmentAxis, AlignmentOptions, AttachmentAxisOptionStatus, ENUM_NAME, OptionEvidence,
};
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

impl<'source> Observed<'_, '_, '_, 'source> {
    pub(in crate::catalog_v3) fn owned_element_ordinal(
        &self,
    ) -> Result<SourcedValue<u32>, CatalogError> {
        let owner = self
            .owner
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        ComponentOwner::ordinal(self.originals, owner)
    }

    pub(in crate::catalog_v3) fn alignment_axis(
        &self,
        field: &str,
        options: &AlignmentOptions<'_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<AlignmentAxis<'source>, CatalogError> {
        let label = self.borrowed_enum_to(field, ENUM_NAME, notes)?;
        let option_status = label
            .value()
            .map(|label| self.alignment_option_status(field, label, options))
            .transpose()?;
        Ok(AlignmentAxis {
            label,
            option_status,
        })
    }

    fn alignment_option_status(
        &self,
        field: &str,
        label: &str,
        options: &AlignmentOptions<'_>,
    ) -> Result<SourcedValue<AttachmentAxisOptionStatus>, CatalogError> {
        let (_, fact) = self
            .originals
            .field(self.owner, field)?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let target = fact.field().storage().target();
        let evidence = options
            .lookup(target, label)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let mut indices = vec![u64::from(target)];
        let (member, state, complete) = match evidence {
            OptionEvidence::Declared(ordinal) => {
                indices.push(ordinal as u64);
                (
                    "optionName",
                    SourceValue::Present(AttachmentAxisOptionStatus::Declared),
                    true,
                )
            }
            OptionEvidence::Undeclared => (
                "optionNames",
                SourceValue::Present(AttachmentAxisOptionStatus::Undeclared),
                true,
            ),
            OptionEvidence::Unreadable(ordinal) => {
                indices.push(ordinal as u64);
                ("optionName", SourceValue::Unreadable, false)
            }
        };
        let option = self.input.origins().dataforge.at(
            SourceLocation::Subresource {
                namespace: "dataforgeEnum".to_owned(),
                indices,
                field: member.to_owned(),
            },
            if complete {
                SourceUseRole::Conversion
            } else {
                SourceUseRole::Attempted
            },
        )?;
        let (declared, declaration) = self.input.declared_enum_name(target)?;
        if declared.ok() != Some(ENUM_NAME) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let mut datum = fact.bind(
            OriginalValuePart::Value,
            state,
            SourceUseRole::Payload,
            complete,
        )?;
        for usage in fact.declaration_selectors(declaration)? {
            datum.contribute(usage);
        }
        datum.contribute(self.kind()?);
        datum.contribute(option);
        Ok(datum.into_cell(self.originals.sources()))
    }
}
