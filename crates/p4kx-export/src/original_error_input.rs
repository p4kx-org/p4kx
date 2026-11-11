// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUseRole, Value, ValueError};

use super::{OriginalErrorParameter, OriginalValueEdge, OriginalValueFact, OriginalValuePart};
use crate::{CatalogError, ExportAccountingError};

impl OriginalValueFact<'_, '_, '_> {
    /// Verifies the selected non-array Class declaration that emitted an inline cycle.
    pub(crate) fn check_inline_class_cycle(&self, index: u32) -> Result<(), CatalogError> {
        let storage = self.field().storage();
        if storage.data_type() != DataType::Class
            || storage.is_array()
            || u32::from(storage.target()) != index
            || !matches!(
                self.value(),
                Value::Error(
                    ValueError::InlineClassCycle { struct_index }
                ) if *struct_index == index
            )
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(())
    }

    /// Selects the actual declaration or decoded input used by a pointer error parameter.
    pub(crate) fn pointer_error_input(
        &self,
        parameter: OriginalErrorParameter,
        value: u32,
    ) -> Result<(OriginalValuePart, SourceUseRole), CatalogError> {
        let storage = self.field().storage();
        let class_element = storage.data_type() == DataType::Class
            && storage.is_array()
            && matches!(self.edge(), OriginalValueEdge::Element { .. });
        match parameter {
            OriginalErrorParameter::StructIndex if class_element => {
                if u32::from(storage.target()) != value {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                Ok((OriginalValuePart::Target, SourceUseRole::Selector))
            }
            OriginalErrorParameter::InstanceIndex if class_element => Ok((
                OriginalValuePart::Error(parameter),
                SourceUseRole::Conversion,
            )),
            OriginalErrorParameter::StructIndex | OriginalErrorParameter::InstanceIndex => {
                Ok((OriginalValuePart::Error(parameter), SourceUseRole::Payload))
            }
            _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }
}
