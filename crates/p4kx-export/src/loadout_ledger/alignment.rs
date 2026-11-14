// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::LoadoutTargetGuid;
use p4kx_extraction::{Guid, SourceUseRole, SourceValue};

use super::Capture;
use crate::catalog_read::original::OriginalDatum;
use crate::guid_text::CatalogGuid;
use crate::loadout::alignment::{AttachmentIdentity, AttachmentProof};
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError};

impl Capture<'_> {
    pub(in crate::loadout) fn attachment_identity(
        &self,
        guid: Guid,
        record: u32,
        selection: Option<&LoadoutTargetGuid>,
    ) -> Result<(AttachmentIdentity, AttachmentProof), CatalogError> {
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let sources = &input.origins().uses;
        let index = input.origins().dataforge.at(
            RecordHeaderField::RecordIndex.location(record),
            SourceUseRole::Selector,
        )?;
        let original = selection
            .map(|selection| self.retained_target(guid, selection))
            .transpose()?
            .flatten();
        let (target_guid, source_uses, source_set_complete) = if let Some(original) = original {
            let (selected, complete) = original.contributions();
            let mut inputs = Vec::with_capacity(selected.len() + 1);
            inputs.extend_from_slice(selected);
            inputs.push(index);
            (
                original.map(CatalogGuid)?.into_cell(sources),
                inputs,
                complete,
            )
        } else {
            (
                sources.value(SourceValue::Present(CatalogGuid(guid)), &[], false),
                vec![index],
                false,
            )
        };
        let identity = AttachmentIdentity {
            target_guid,
            target_record_index: sources.value(
                SourceValue::Present(record),
                &source_uses,
                source_set_complete,
            ),
        };
        Ok((
            identity,
            AttachmentProof {
                source_uses,
                source_set_complete,
            },
        ))
    }

    fn retained_target(
        &self,
        guid: Guid,
        selection: &LoadoutTargetGuid,
    ) -> Result<Option<OriginalDatum<Guid>>, CatalogError> {
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        if selection.guid() != guid {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let receipt = match (selection.selection(), selection.class_selection()) {
            (Some(reference), None) => self.lock()?.references.get(reference, guid)?,
            (None, Some(class)) => self.class_target(class, guid, selection.class_xml_use())?,
            (None, None) => return Ok(None),
            (Some(_), Some(_)) => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        receipt.retained_for(input).map(Some)
    }
}
