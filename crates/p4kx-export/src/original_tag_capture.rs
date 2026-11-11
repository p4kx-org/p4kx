// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUse, SourceUseRole, SourceValue};

use super::{
    OriginalErrorParameter, OriginalReferenceObservation, OriginalValueFact, OriginalValuePart,
};
use crate::CatalogError;
use crate::catalog_v3::tag_catalog::{CapturedTags, TagDatum, TagPathLookup, TagPathMeaning};
use crate::record_header_field::RecordHeaderField;

impl<'source> OriginalReferenceObservation<'_, '_, '_, 'source> {
    pub(crate) fn tag_path(
        &self,
        tags: &CapturedTags<'_, 'source>,
    ) -> Result<TagPathLookup, CatalogError> {
        let mut lookup = tags.lookup(self)?;
        let (meaning, datum) = match &mut lookup {
            TagPathLookup::Ready { meaning, value } => (*meaning, value),
            TagPathLookup::Unavailable { value } => (TagPathMeaning::Path, value),
        };
        datum.contribute(self.tag_selection_inputs(meaning)?);
        Ok(lookup)
    }

    fn tag_selection_inputs(
        &self,
        meaning: TagPathMeaning,
    ) -> Result<Vec<SourceUse>, CatalogError> {
        let fact = self.caller();
        let admitted = self.admitted();
        let guid_role = if matches!(meaning, TagPathMeaning::GuidFallback) {
            SourceUseRole::Payload
        } else {
            SourceUseRole::Selector
        };
        let caller_role = if admitted.storage == DataType::WeakPointer {
            SourceUseRole::Selector
        } else {
            guid_role
        };
        let mut inputs = vec![fact.observe(OriginalValuePart::Value, caller_role)?];
        if admitted.storage == DataType::WeakPointer {
            inputs.push(fact.observe(OriginalValuePart::Value, SourceUseRole::Conversion)?);
            if admitted.reference.location().is_none() {
                inputs.push(fact.scope().dataforge().at(
                    RecordHeaderField::Guid.location(admitted.origin.record_index()),
                    guid_role,
                )?);
            }
        }
        Ok(inputs)
    }
}

impl OriginalValueFact<'_, '_, '_> {
    /// Keeps the authored GUID while representing the resolved-only lookup's missing target.
    pub(crate) fn dangling_tag(&self) -> Result<Option<TagPathLookup>, CatalogError> {
        let Some(guid) = self.dangling_guid(SourceUseRole::Selector)? else {
            return Ok(None);
        };
        let (inputs, complete) = guid.contributions();
        let mut inputs = inputs.to_vec();
        inputs.push(self.observe(
            OriginalValuePart::Error(OriginalErrorParameter::Guid),
            SourceUseRole::Conversion,
        )?);
        Ok(Some(TagPathLookup::Ready {
            meaning: TagPathMeaning::Unresolved,
            value: TagDatum::local(SourceValue::Absent, inputs, complete),
        }))
    }
}
