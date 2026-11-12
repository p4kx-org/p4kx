// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUse, SourceUses, SourcedValue};

use super::{Fact, Observed, ReferenceCardinality, ReferenceCell, decode_weak_link};
use crate::CatalogError;
use crate::catalog_read::original::OriginalValuePart;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceWeakReference;

type LinkPayload<T> = Option<(OriginalValuePart, T)>;
type LinkList<T> = SourcedValue<Vec<SourcedValue<T>>>;

impl<'scope, 'read, 'source> Observed<'_, 'scope, 'read, 'source> {
    pub(in crate::catalog_v3) fn record_link_array_to(
        &self,
        name: &str,
        target_type: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Vec<ReferenceCell<'source>>>, CatalogError> {
        self.link_array_to(
            name,
            (DataType::Reference, target_type),
            ["a nullable Reference array", "a nullable Reference"],
            notes,
            |fact, sources, selectors| self.decode_record_link(fact, sources, selectors),
        )
    }

    /// Reads every original `WeakPointer` slot in its declared array.
    pub(in crate::catalog_v3) fn weak_link_array_to(
        &self,
        name: &str,
        target_type: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<LinkList<Box<SourceWeakReference<'source>>>, CatalogError> {
        self.link_array_to(
            name,
            (DataType::WeakPointer, target_type),
            ["a nullable WeakPointer array", "a nullable WeakPointer"],
            notes,
            |fact, sources, _| decode_weak_link(fact, sources),
        )
    }

    fn link_array_to<T>(
        &self,
        name: &str,
        declaration: (DataType, &str),
        [array, element]: [&'static str; 2],
        notes: &mut Vec<FieldError>,
        decode: impl Fn(
            &Fact<'scope, 'read, 'source>,
            &SourceUses,
            &mut Vec<SourceUse>,
        ) -> Result<LinkPayload<T>, CatalogError>,
    ) -> Result<LinkList<T>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let mut selectors = Vec::new();
        if let Some(rejected) = self.reject_link_declaration(
            fact,
            declaration,
            ReferenceCardinality::Array,
            notes,
            &mut selectors,
        )? {
            return Ok(rejected);
        }
        let state = self.array_elements(fact, name, array, notes, |index, fact, notes| {
            let mut selectors = Vec::new();
            let decoded = decode(fact, self.originals.sources(), &mut selectors)?;
            let (part, state) =
                self.link_state(&format!("{name}[{index}]"), fact, decoded, element, notes)?;
            self.bind_link(fact, part, state, selectors)
        })?;
        self.bind_link(fact, OriginalValuePart::Value, state, selectors)
    }
}
