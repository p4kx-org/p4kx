// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, Value};

use super::health_reference_receipt::{ReferenceDatum, ReferenceUse};
use super::health_text_capture::HealthTextCapture;
use super::{OriginalRecordScope, OriginalValueEdge, OriginalValuePart, OriginalValueSite};
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, ExportAccountingError, HealthText};

struct PathFrame {
    ordinal: u64,
    restore: usize,
    owner_type: ReferenceDatum<HealthText>,
}

/// Actual declaration and array-selection inputs along the current original owned path.
pub(super) struct ReferencePath {
    inputs: Vec<ReferenceUse>,
    frames: Vec<PathFrame>,
    owner_type: ReferenceDatum<HealthText>,
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
            owner_type: ReferenceDatum::captured(
                SourceValue::Present(original_type),
                vec![ReferenceUse::captured(
                    RecordHeaderField::SourceType.location(scope.record().index()),
                    SourceUseRole::Selector,
                )],
            ),
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

    pub(super) fn field(
        &self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
        texts: &HealthTextCapture,
    ) -> Result<ReferenceDatum<HealthText>, CatalogError> {
        let mut inputs = self.inputs.clone();
        add_field(&mut inputs, site);
        let text = texts.retain_text(site.logical_path())?.text().clone();
        Ok(ReferenceDatum::captured(SourceValue::Present(text), inputs))
    }

    pub(super) fn owner_type(
        &self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
    ) -> Result<ReferenceDatum<HealthText>, ExportAccountingError> {
        if !matches!(self.owner_type.value(), SourceValue::Present(value) if value.as_str() == site.owner_type())
        {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(self.owner_type.clone())
    }

    pub(super) fn enter(
        &mut self,
        site: &OriginalValueSite<'_, '_, '_, '_>,
        texts: &HealthTextCapture,
    ) -> Result<(), CatalogError> {
        let restore = self.inputs.len();
        let owner_type = self.owner_type.clone();
        add_field(&mut self.inputs, site);
        match site.value() {
            Value::Struct(value) => {
                let text = texts.retain_text(value.type_name())?.text().clone();
                self.owner_type = ReferenceDatum::captured(
                    SourceValue::Present(text),
                    vec![ReferenceUse::captured(
                        site.fact().location(OriginalValuePart::ConcreteType),
                        SourceUseRole::Selector,
                    )],
                );
            }
            Value::Array(_) => self.inputs.push(ReferenceUse::captured(
                site.fact().location(OriginalValuePart::Value),
                SourceUseRole::Selector,
            )),
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
        self.frames.push(PathFrame {
            ordinal: site.ordinal(),
            restore,
            owner_type,
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
        self.inputs.truncate(frame.restore);
        self.owner_type = frame.owner_type;
        Ok(())
    }
}

fn add_field(inputs: &mut Vec<ReferenceUse>, site: &OriginalValueSite<'_, '_, '_, '_>) {
    if matches!(site.edge(), OriginalValueEdge::Field { .. }) {
        inputs.push(ReferenceUse::captured(
            site.fact().location(OriginalValuePart::FieldName),
            SourceUseRole::Conversion,
        ));
    }
}
