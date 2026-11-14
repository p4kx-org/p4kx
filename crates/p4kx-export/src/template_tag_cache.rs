// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashMap;

use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::catalog_v3::tag_catalog::{CapturedTags, TagPathLookup};
use crate::template_reference_index::TemplateReferences;
use crate::{CatalogError, ExportAccountingError};

/// One actual Tag selection per original caller in a single bound Template frame.
pub(crate) struct TemplateTagCache<'context, 'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    tags: &'context CapturedTags<'context, 'source>,
    found: RefCell<HashMap<u64, TagPathLookup>>,
}

impl<'context, 'fields, 'scope, 'read, 'source>
    TemplateTagCache<'context, 'fields, 'scope, 'read, 'source>
{
    pub(crate) fn new(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        tags: &'context CapturedTags<'context, 'source>,
    ) -> Self {
        Self {
            fields,
            tags,
            found: RefCell::new(HashMap::new()),
        }
    }

    pub(crate) fn tree(&self) -> &p4kx_extraction::TagTree {
        self.tags.tree()
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
        let value = match fact.dangling_tag()? {
            Some(value) => value,
            None => references.observation(fact)?.tag_path(self.tags)?,
        };
        found.insert(fact.ordinal(), value.clone());
        Ok(value)
    }
}
