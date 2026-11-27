// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourcedValue, StructValue, Value};

use super::Observed;
use crate::CatalogError;
use crate::record_fields::{self, structure, text};
use crate::source_fields::FieldError;

pub(super) const COMPONENT: &str = "SAttachableComponentParams";
const DEFINITION: &str = "AttachDef";
const TYPE: &str = "Type";

pub(crate) fn first<'read, 'source>(
    tree: &'read RecordTree<'source>,
) -> Option<(&'read Value<'source>, &'read StructValue<'source>)> {
    record_fields::component_occurrence(tree, COMPONENT)
}

pub(crate) fn fields<'read, 'source>(
    tree: &'read RecordTree<'source>,
) -> Option<&'read StructValue<'source>> {
    structure(first(tree).map(|(_, fields)| fields), DEFINITION)
}

pub(crate) fn raw_type<'source>(tree: &RecordTree<'source>) -> Option<&'source str> {
    text(fields(tree), TYPE)
}

pub(super) struct AttachmentDefinition<'reader, 'scope, 'read, 'source> {
    observed: Observed<'reader, 'scope, 'read, 'source>,
}

impl<'reader, 'scope, 'read, 'source> AttachmentDefinition<'reader, 'scope, 'read, 'source> {
    pub(super) fn read<T>(
        component: Observed<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
        decode: impl FnOnce(&Self, &mut Vec<FieldError>) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        component.declared_owned(DEFINITION, notes, |observed, notes| {
            decode(&Self { observed }, notes)
        })
    }

    pub(super) fn item_type(
        &self,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'source str>, CatalogError> {
        self.observed.borrowed_label(TYPE, notes)
    }

    pub(super) fn tags(
        &self,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'source str>, CatalogError> {
        self.observed.borrowed_label("Tags", notes)
    }

    pub(super) fn required_tags(
        &self,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'source str>, CatalogError> {
        self.observed.borrowed_label("RequiredTags", notes)
    }

    pub(super) const fn observed(&self) -> Observed<'reader, 'scope, 'read, 'source> {
        self.observed
    }
}

#[cfg(test)]
#[path = "item_attachment_tests.rs"]
mod tests;
