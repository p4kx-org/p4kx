// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceUses, SourceValue};

use super::OriginalDatum;
use super::health_example_body::{ExampleBody, ExampleBodyReceipt};
use super::health_example_cells::ExampleBodyCells;
use super::health_text_capture::{CapturedText, SealedTexts};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError, HealthText};

/// Only retained source recipes materialized before this report's one text seal.
pub(super) struct RetainedExampleBody {
    field: OriginalDatum<CapturedText>,
    source_type: OriginalDatum<CapturedText>,
    target_guid: OriginalDatum<Guid>,
}

impl RetainedExampleBody {
    pub(super) fn read(
        receipt: ExampleBodyReceipt,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<Self, CatalogError> {
        receipt.check_input(input)?;
        let texts = input.origins().health_texts();
        match receipt.into_body() {
            ExampleBody::Header(recipe) => {
                let facts = recipe.retain(input)?;
                Ok(Self {
                    field: texts.capture_shared(facts.field)?,
                    source_type: facts.source_type,
                    target_guid: facts.target_guid,
                })
            }
            ExampleBody::Captured {
                frame,
                field,
                source_type,
                target_guid,
            } => Ok(Self {
                field: texts.capture_shared(field.finish(&frame)?)?,
                source_type: texts.capture_shared(source_type.finish(&frame)?)?,
                target_guid: target_guid.finish(&frame)?,
            }),
            ExampleBody::Technical { subject, field } => Ok(Self {
                field: texts.capture_shared(OriginalDatum::observed(
                    SourceValue::Present(field),
                    Vec::new(),
                    true,
                ))?,
                source_type: subject.retain_source_type(&input.origins().uses)?,
                target_guid: OriginalDatum::observed(SourceValue::Absent, Vec::new(), true),
            }),
            ExampleBody::Loadout(receipt) => Ok(Self {
                field: texts.capture_shared(receipt.path_fact()?)?,
                source_type: OriginalDatum::observed(SourceValue::Absent, Vec::new(), true),
                target_guid: OriginalDatum::observed(SourceValue::Absent, Vec::new(), true),
            }),
            ExampleBody::RequestedLoadout(receipt) => Ok(Self {
                field: texts.capture_shared(receipt.retain_field()?)?,
                source_type: receipt
                    .subject()
                    .retain_source_type(&input.origins().uses)?,
                target_guid: OriginalDatum::observed(SourceValue::Absent, Vec::new(), true),
            }),
            ExampleBody::LoadoutTarget(receipt) => Ok(Self {
                field: OriginalDatum::observed(SourceValue::Absent, Vec::new(), true),
                source_type: receipt
                    .subject()
                    .retain_source_type(&input.origins().uses)?,
                target_guid: receipt.retain_target()?,
            }),
            ExampleBody::GuidHeader(subject) => Ok(Self {
                field: OriginalDatum::observed(SourceValue::Absent, Vec::new(), true),
                source_type: OriginalDatum::observed(SourceValue::Absent, Vec::new(), true),
                target_guid: subject.retain_guid(&input.origins().uses)?,
            }),
            ExampleBody::Absent(_) => Ok(Self {
                field: OriginalDatum::observed(SourceValue::Absent, Vec::new(), true),
                source_type: OriginalDatum::observed(SourceValue::Absent, Vec::new(), true),
                target_guid: OriginalDatum::observed(SourceValue::Absent, Vec::new(), true),
            }),
        }
    }

    pub(super) const fn texts(&self) -> [&OriginalDatum<CapturedText>; 2] {
        [&self.field, &self.source_type]
    }

    pub(super) fn finish(
        self,
        texts: &SealedTexts,
        sources: &SourceUses,
    ) -> Result<ExampleBodyCells, ExportAccountingError> {
        Ok(ExampleBodyCells {
            field: texts.cell(self.field, sources)?,
            source_type: texts.cell(self.source_type, sources)?,
            target_guid: self
                .target_guid
                .map(|guid| HealthText::new(&guid.to_string()))?
                .into_cell(sources),
        })
    }
}
