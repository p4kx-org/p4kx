// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::OnceCell;
use std::sync::Arc;

use p4kx_extraction::{SourceUseRole, Value};

use super::health_reference_receipt::ReferenceUse;
use super::health_text_capture::HealthTextCapture;
use super::{OriginalRecordScope, OriginalValueEdge, OriginalValuePart, OriginalValueSite};
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError, HealthText};

struct PathFrame {
    ordinal: u64,
    restore: usize,
    prefix: OnceCell<Arc<[ReferenceUse]>>,
    owner: ReferenceOwner,
}

#[derive(Debug, Clone)]
pub(super) struct ReferenceOwner {
    pub(super) type_name: HealthText,
    pub(super) input: ReferenceUse,
}

#[derive(Debug, Clone)]
pub(super) struct ReferencePathReceipt {
    pub(super) field: HealthText,
    pub(super) uses: ReferencePathUses,
    pub(super) owner: ReferenceOwner,
}

/// The original container operands followed by the named leaf's declaration, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReferencePathUses {
    prefix: Option<Arc<[ReferenceUse]>>,
    leaf: Option<ReferenceUse>,
}

impl ReferencePathUses {
    pub(super) fn into_vec(self) -> Vec<ReferenceUse> {
        let prefix = self.prefix.as_deref().unwrap_or_default();
        let mut inputs = Vec::with_capacity(prefix.len() + usize::from(self.leaf.is_some()));
        inputs.extend_from_slice(prefix);
        inputs.extend(self.leaf);
        inputs
    }

    #[cfg(test)]
    pub(super) fn shares_prefix_with(&self, other: &Self) -> bool {
        matches!((&self.prefix, &other.prefix), (Some(left), Some(right)) if Arc::ptr_eq(left, right))
    }
}

/// Actual declaration and array-selection inputs along the current original owned path.
pub(super) struct ReferencePath {
    inputs: Vec<ReferenceUse>,
    frames: Vec<PathFrame>,
    prefix: OnceCell<Arc<[ReferenceUse]>>,
    owner: ReferenceOwner,
}

impl ReferencePath {
    pub(super) fn new(
        scope: &OriginalRecordScope<'_, '_>,
        texts: &HealthTextCapture,
    ) -> Result<Self, CatalogError> {
        let original_type = texts
            .retain_text(scope.tree().root().type_name())?
            .text()
            .clone();
        Ok(Self {
            inputs: Vec::new(),
            frames: Vec::new(),
            prefix: OnceCell::new(),
            owner: ReferenceOwner {
                type_name: original_type,
                input: ReferenceUse::header(
                    scope.record().index(),
                    RecordHeaderField::SourceType,
                    SourceUseRole::Selector,
                ),
            },
        })
    }

    pub(super) fn check_site(
        &self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
    ) -> Result<(), ExportAccountingError> {
        if site.parent_ordinal() != self.frames.last().map(|frame| frame.ordinal) {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(())
    }

    pub(super) fn receipt(
        &self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
        texts: &HealthTextCapture,
    ) -> Result<ReferencePathReceipt, CatalogError> {
        let field = texts.retain_text(site.logical_path())?.text().clone();
        if self.owner.type_name.as_str() != site.owner_type() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let prefix = (!self.inputs.is_empty()).then(|| {
            Arc::clone(
                self.prefix
                    .get_or_init(|| Arc::from(self.inputs.as_slice())),
            )
        });
        Ok(ReferencePathReceipt {
            field,
            uses: ReferencePathUses {
                prefix,
                leaf: field_use(site),
            },
            owner: self.owner.clone(),
        })
    }

    pub(super) fn enter(
        &mut self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
        texts: &HealthTextCapture,
    ) -> Result<(), CatalogError> {
        let restore = self.inputs.len();
        let owner = self.owner.clone();
        let selection = match site.value() {
            Value::Struct(value) => {
                let text = texts.retain_text(value.type_name())?.text().clone();
                self.owner = ReferenceOwner {
                    type_name: text,
                    input: ReferenceUse::for_fact(
                        &site.fact(),
                        OriginalValuePart::ConcreteType,
                        SourceUseRole::Selector,
                    ),
                };
                None
            }
            Value::Array(_) => Some(ReferenceUse::for_fact(
                &site.fact(),
                OriginalValuePart::Value,
                SourceUseRole::Selector,
            )),
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        self.inputs.extend(field_use(site));
        self.inputs.extend(selection);
        let prefix = if self.inputs.len() == restore {
            // An unnamed Struct element changes the owner without adding a path operand.
            OnceCell::new()
        } else {
            std::mem::take(&mut self.prefix)
        };
        self.frames.push(PathFrame {
            ordinal: site.ordinal(),
            restore,
            prefix,
            owner,
        });
        Ok(())
    }

    pub(super) fn leave(&mut self, ordinal: u64) -> Result<(), ExportAccountingError> {
        if self
            .frames
            .last()
            .is_none_or(|frame| frame.ordinal != ordinal)
        {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        let frame = self
            .frames
            .pop()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if self.inputs.len() != frame.restore {
            self.inputs.truncate(frame.restore);
            self.prefix = frame.prefix;
        }
        self.owner = frame.owner;
        Ok(())
    }
}

fn field_use(site: &OriginalValueSite<'_, '_, '_, '_>) -> Option<ReferenceUse> {
    matches!(site.edge(), OriginalValueEdge::Field { .. }).then(|| {
        ReferenceUse::for_fact(
            &site.fact(),
            OriginalValuePart::FieldName,
            SourceUseRole::Conversion,
        )
    })
}
