// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, RecordTree, SourceValueState, SourcedValue};

use super::super::super::Observed;
use super::super::selection::SelectedSource;
use super::{VariantOperands, read_selected};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalFlowFields;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;
use crate::variants::ShipDefinitionKey;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DefinitionMembership {
    Comparable(ShipDefinitionKey),
    Excluded,
    Unavailable,
}

impl DefinitionMembership {
    pub(crate) fn of(original: &SourcedValue<&str>) -> Self {
        if !original.origin().source_set_complete() {
            return Self::Unavailable;
        }
        match original.state() {
            SourceValueState::Present => match original.value().copied() {
                Some("") => Self::Excluded,
                Some(path) => Self::Comparable(ShipDefinitionKey::new(path)),
                None => Self::Unavailable,
            },
            SourceValueState::Absent | SourceValueState::Null => Self::Excluded,
            _ => Self::Unavailable,
        }
    }
}

/// Reads the original ranking operands without preparing text or vehicle dependencies.
pub(crate) fn read<'read, 'source>(
    tree: &'read RecordTree<'source>,
    input: &'read OriginalCatalogInput<'read, 'source>,
    admission: &RecordObservation<'read, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<VariantOperands<'read, 'source>, CatalogError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, input, &capture, &numbers);
    let vehicle = SelectedSource::read(&originals, "Components", crate::ship::VEHICLE_COMPONENT)?;
    let crew = SelectedSource::read(
        &originals,
        "Components",
        "SEntityComponentCrewManifestParams",
    )?;
    read_selected(fields, input, admission, &vehicle, &crew, &mut Vec::new())
}
