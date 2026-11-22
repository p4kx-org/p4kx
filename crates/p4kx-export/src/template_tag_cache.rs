// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;

use p4kx_extraction::{CancelToken, DataForge};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::catalog_v3::tag_catalog::{self, CapturedTags, TagContextRead, TagPathLookup};
use crate::template_reference_index::TemplateReferences;
use crate::{CatalogError, ExportAccountingError};

#[derive(Clone, Copy)]
pub(crate) enum TemplateTagInput<'context, 'source> {
    Prepared(&'context CapturedTags<'context, 'source>),
    OnDemand(&'context dyn DemandTagContext<'source>),
}

impl<'context, 'source> TemplateTagInput<'context, 'source> {
    fn capture(self) -> Result<&'context CapturedTags<'context, 'source>, CatalogError> {
        match self {
            Self::Prepared(tags) => Ok(tags),
            Self::OnDemand(tags) => tags.capture(),
        }
    }
}

pub(crate) trait DemandTagContext<'source> {
    fn capture(&self) -> Result<&CapturedTags<'_, 'source>, CatalogError>;
}

pub(crate) struct DeferredTemplateTags<'context, 'source> {
    forge: &'source DataForge<'source>,
    input: &'context OriginalCatalogInput<'context, 'source>,
    cancel: Option<&'context CancelToken>,
    read: OnceCell<TagContextRead<'context, 'source>>,
}

impl<'context, 'source> DeferredTemplateTags<'context, 'source> {
    pub(crate) const fn new(
        forge: &'source DataForge<'source>,
        input: &'context OriginalCatalogInput<'context, 'source>,
        cancel: Option<&'context CancelToken>,
    ) -> Self {
        Self {
            forge,
            input,
            cancel,
            read: OnceCell::new(),
        }
    }

    pub(crate) fn was_read(&self) -> bool {
        self.read.get().is_some()
    }

    pub(crate) fn into_failures(self) -> Vec<crate::CatalogReadFailure<'source>> {
        self.read
            .into_inner()
            .map_or_else(Vec::new, |read| read.failures)
    }
}

impl<'source> DemandTagContext<'source> for DeferredTemplateTags<'_, 'source> {
    fn capture(&self) -> Result<&CapturedTags<'_, 'source>, CatalogError> {
        if let Some(read) = self.read.get() {
            return Ok(&read.context);
        }
        let read = tag_catalog::read_context(self.forge, self.input, self.cancel)?;
        self.read
            .set(read)
            .map_err(|_| ExportAccountingError::ConflictingTagTrace)?;
        self.read
            .get()
            .map(|read| &read.context)
            .ok_or_else(|| ExportAccountingError::ConflictingTagTrace.into())
    }
}

/// One actual Tag selection per original caller in a single bound Template frame.
pub(crate) struct TemplateTagCache<'context, 'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    tags: TemplateTagInput<'context, 'source>,
    found: RefCell<HashMap<u64, TagPathLookup>>,
}

impl<'context, 'fields, 'scope, 'read, 'source>
    TemplateTagCache<'context, 'fields, 'scope, 'read, 'source>
{
    pub(crate) fn new(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        tags: TemplateTagInput<'context, 'source>,
    ) -> Self {
        Self {
            fields,
            tags,
            found: RefCell::new(HashMap::new()),
        }
    }

    pub(crate) fn lookup(
        &self,
        references: &RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
        fact: &'fields OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<TagPathLookup, CatalogError> {
        self.fields.span(fact)?;
        let mut found = self
            .found
            .try_borrow_mut()
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        if let Some(value) = found.get(&fact.ordinal()) {
            return Ok(value.clone());
        }
        let mut references = references
            .try_borrow_mut()
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        let value = if let Some(value) = fact.dangling_tag()? {
            value
        } else {
            let observed = references.observation(fact)?;
            if matches!(self.tags, TemplateTagInput::OnDemand(_))
                && observed.reference().location().is_some()
            {
                observed.local_tag_path()?
            } else {
                observed.tag_path(self.tags.capture()?)?
            }
        };
        found.insert(fact.ordinal(), value.clone());
        Ok(value)
    }
}
