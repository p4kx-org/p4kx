// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceUseRole, SourceValue, SourcedValue, Value};

use super::Observed;
use crate::CatalogError;
use crate::catalog_read::original::OriginalValuePart;
use crate::harvest_link::MINEABLE_PARAMS;
use crate::record_fields;
use crate::source_fields::FieldError;

impl<'source> Observed<'_, '_, '_, 'source> {
    pub(in crate::catalog_v3) fn harvest_text(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<String>, CatalogError> {
        self.scalar(name, "a nullable String or Enum", notes, |fact| {
            Ok(match fact.value() {
                Value::String(text) | Value::Enum(text) => Some((*text).to_owned()),
                _ => None,
            })
        })
    }

    pub(in crate::catalog_v3) fn harvest_paths(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<String>>>, CatalogError> {
        let Some((_, fact)) = self.originals.field(self.owner, name)? else {
            return self.absent();
        };
        let field = self.array_elements(
            fact,
            name,
            "a nullable String array",
            notes,
            |index, part, notes| {
                let (state, role) = match part.value() {
                    Value::String(text) => (
                        SourceValue::Present((*text).to_owned()),
                        SourceUseRole::Payload,
                    ),
                    Value::Null => (SourceValue::Null, SourceUseRole::Selector),
                    value => (
                        self.failure(
                            &format!("{name}[{index}]"),
                            value,
                            "a nullable String",
                            notes,
                        )?,
                        SourceUseRole::Selector,
                    ),
                };
                self.bind_field(part, OriginalValuePart::Value, state, role)
            },
        )?;
        self.bind_storage(fact, field)
    }

    pub(in crate::catalog_v3) fn harvest_mineable<T>(
        &self,
        tree: &RecordTree<'source>,
        read: impl FnOnce(Self) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let selected = record_fields::component_occurrence(tree, MINEABLE_PARAMS);
        let evidence = self
            .originals
            .component_selection(selected.map(|(value, _)| value))?;
        let state = match selected {
            Some((value, _)) => {
                let owner = self.originals.fact(value)?;
                SourceValue::Present(read(self.at_owner(owner))?)
            }
            None if evidence.complete => SourceValue::Absent,
            None => SourceValue::Unreadable,
        };
        Ok(self
            .originals
            .sources()
            .value(state, &evidence.uses, evidence.complete))
    }
}
