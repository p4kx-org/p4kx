// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::LoadoutTargetGuid;
use p4kx_extraction::{Guid, SourceValue, SourcedValue};

use super::Capture;
use crate::guid_text::CatalogGuid;
use crate::{CatalogError, ExportAccountingError};

impl Capture<'_> {
    pub(in crate::loadout) fn attachment_target(
        &self,
        guid: Guid,
        selection: Option<&LoadoutTargetGuid>,
    ) -> Result<SourcedValue<CatalogGuid>, CatalogError> {
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let sources = &input.origins().uses;
        let Some(selection) = selection else {
            return Ok(sources.value(SourceValue::Present(CatalogGuid(guid)), &[], false));
        };
        if selection.guid() != guid {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let receipt = match (selection.selection(), selection.class_selection()) {
            (Some(reference), None) => self.lock()?.references.get(reference, guid)?,
            (None, Some(class)) => self.class_target(class, guid, selection.class_xml_use())?,
            (None, None) => {
                return Ok(sources.value(SourceValue::Present(CatalogGuid(guid)), &[], false));
            }
            (Some(_), Some(_)) => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        Ok(receipt
            .retained_for(input)?
            .map(CatalogGuid)?
            .into_cell(sources))
    }
}
