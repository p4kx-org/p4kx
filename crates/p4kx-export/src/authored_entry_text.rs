// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{InputScope, SourceInput, SourceUse, SourceUseRole, SourceValue};

use super::OriginalDatum;
use super::entry_selection_input::{EntrySelectionUse, SelectionSource};
use super::health_entry_frame::{EntryCapture, EntryFrame};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError, HealthText};

/// Original text paired with the actual source operations that supplied it.
#[derive(Debug, Clone)]
pub(super) struct AuthoredEntryText {
    frame: Arc<EntryFrame>,
    text: HealthText,
    inputs: Arc<[EntrySelectionUse]>,
    xml_operation: Option<p4kx_assembly::LoadoutFileObservation>,
}

#[derive(Clone, Copy)]
enum TextContribution {
    Original,
    Selector,
}

impl AuthoredEntryText {
    pub(super) fn captured(
        capture: &EntryCapture<'_>,
        text: &str,
        inputs: Vec<EntrySelectionUse>,
    ) -> Result<Self, CatalogError> {
        let frame = capture.frame();
        for input in &inputs {
            match &input.source {
                SelectionSource::Archive(origin) => frame.check_origin(origin)?,
                SelectionSource::Entry(entry) => entry.check_frame(&frame)?,
                SelectionSource::Record(frame) => capture.check_record_frame(frame)?,
                SelectionSource::UnboundLoadout(_) => {}
            }
        }
        Ok(Self {
            frame,
            text: capture.text(text)?,
            inputs: Arc::from(inputs),
            xml_operation: None,
        })
    }

    pub(super) fn with_xml_operation(
        mut self,
        operation: &p4kx_assembly::LoadoutFileObservation,
    ) -> Self {
        self.xml_operation = Some(operation.clone());
        self
    }

    pub(super) fn is_from_xml(&self, operation: &p4kx_assembly::LoadoutFileObservation) -> bool {
        self.xml_operation
            .as_ref()
            .is_some_and(|original| original.is_same_operation(operation))
    }

    pub(super) fn is_same_recipe(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.frame, &other.frame)
            && Arc::ptr_eq(&self.inputs, &other.inputs)
            && self.text == other.text
    }

    pub(super) fn frame(&self) -> Arc<EntryFrame> {
        Arc::clone(&self.frame)
    }

    pub(super) fn as_str(&self) -> &str {
        self.text.as_str()
    }
    pub(super) const fn text(&self) -> &HealthText {
        &self.text
    }

    pub(super) fn derived<'text>(
        capture: &EntryCapture<'_>,
        inputs: impl IntoIterator<Item = &'text Self>,
        value: &str,
    ) -> Result<Self, CatalogError> {
        let mut contributions = Vec::new();
        for input in inputs {
            input.frame.check_frame(&capture.frame())?;
            contributions.extend(input.inputs.iter().cloned());
            contributions.extend(
                input
                    .inputs
                    .iter()
                    .filter(|input| input.role == SourceUseRole::Payload)
                    .map(|input| EntrySelectionUse {
                        source: input.source.clone(),
                        location: input.location.clone(),
                        role: SourceUseRole::Conversion,
                    }),
            );
        }
        Self::captured(capture, value, contributions)
    }

    pub(super) fn selector_inputs(&self) -> Result<(Vec<SourceUse>, bool), CatalogError> {
        let inputs = self
            .inputs
            .iter()
            .map(|input| self.materialize(input, TextContribution::Selector))
            .collect::<Result<_, _>>()?;
        Ok((inputs, self.complete()))
    }

    fn complete(&self) -> bool {
        !self
            .inputs
            .iter()
            .any(|input| matches!(&input.source, SelectionSource::UnboundLoadout(_)))
    }

    fn materialize(
        &self,
        input: &EntrySelectionUse,
        contribution: TextContribution,
    ) -> Result<SourceUse, CatalogError> {
        let role = match contribution {
            TextContribution::Selector => SourceUseRole::Selector,
            TextContribution::Original => input.role,
        };
        match &input.source {
            SelectionSource::Archive(origin) => {
                self.frame.check_origin(origin)?;
                Ok(self
                    .frame
                    .sources()
                    .observe(SourceInput::Archive(origin), InputScope::Payload)?
                    .at(input.location.clone(), role)?)
            }
            SelectionSource::Entry(entry) => {
                entry.check_frame(&self.frame)?;
                entry.payload_at(input.location.clone(), role)
            }
            SelectionSource::Record(frame) => Ok(frame.at(input.location.clone(), role)?),
            SelectionSource::UnboundLoadout(operation) => {
                if operation.source().archive_input_origin().is_some()
                    || operation.read_outcome().is_err()
                {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                Ok(self
                    .frame
                    .sources()
                    .observe(SourceInput::Unknown, InputScope::Payload)?
                    .at(input.location.clone(), role)?)
            }
        }
    }

    pub(super) fn convert(
        mut self,
        capture: &EntryCapture<'_>,
        convert: impl FnOnce(&str) -> Option<String>,
    ) -> Result<Option<Self>, CatalogError> {
        self.frame.check_frame(&capture.frame())?;
        let Some(value) = convert(self.as_str()) else {
            return Ok(None);
        };
        let conversions: Vec<_> = self
            .inputs
            .iter()
            .filter(|input| input.role == SourceUseRole::Payload)
            .map(|input| EntrySelectionUse {
                source: input.source.clone(),
                location: input.location.clone(),
                role: SourceUseRole::Conversion,
            })
            .collect();
        let mut inputs: Vec<_> = self.inputs.iter().cloned().collect();
        inputs.extend(conversions);
        self.inputs = Arc::from(inputs);
        self.text = capture.text(&value)?;
        Ok(Some(self))
    }

    pub(super) fn selected_by(mut self, selector: &Self) -> Result<Self, ExportAccountingError> {
        self.frame.check_frame(&selector.frame)?;
        let mut inputs: Vec<_> = self.inputs.iter().cloned().collect();
        inputs.extend(selector.inputs.iter().map(|input| EntrySelectionUse {
            source: input.source.clone(),
            location: input.location.clone(),
            role: if input.role == SourceUseRole::Payload {
                SourceUseRole::Selector
            } else {
                input.role
            },
        }));
        self.inputs = Arc::from(inputs);
        Ok(self)
    }

    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        self.frame.check_input(input)?;
        for contribution in self.inputs.iter() {
            match &contribution.source {
                SelectionSource::Record(frame) => frame.check_input(input)?,
                SelectionSource::Entry(entry) => entry.check_frame(&self.frame)?,
                SelectionSource::Archive(_) | SelectionSource::UnboundLoadout(_) => {}
            }
        }
        Ok(())
    }

    pub(super) fn retain(&self) -> Result<OriginalDatum<HealthText>, CatalogError> {
        let mut inputs: Vec<SourceUse> = Vec::new();
        for input in self.inputs.iter() {
            inputs.push(self.materialize(input, TextContribution::Original)?);
        }
        let complete = self.complete();
        Ok(OriginalDatum::observed(
            SourceValue::Present(self.text.clone()),
            inputs,
            complete,
        ))
    }
}
