// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};

use crate::record_read::ReadError;
use crate::source_fields::FieldCapture;

/// The same `ScopedLocation` guards with the actual original error-capture receiver.
pub(crate) struct ShapeFields<'capture, 'tree, 'source> {
    source: &'tree StructValue<'source>,
    capture: &'capture dyn FieldCapture<'source>,
}

impl<'capture, 'tree, 'source> ShapeFields<'capture, 'tree, 'source> {
    pub(crate) fn new(
        source: &'tree StructValue<'source>,
        capture: &'capture dyn FieldCapture<'source>,
    ) -> Result<Self, ReadError> {
        capture.check_parent(source)?;
        Ok(Self { source, capture })
    }

    pub(crate) fn structure(
        &self,
        field: &'static str,
    ) -> Result<Option<&'tree StructValue<'source>>, ReadError> {
        super::structure(self.source, field, Some(self.capture))
    }

    pub(crate) fn enum_view(&self, field: &'static str) -> Result<(), ReadError> {
        super::enum_context(self.source, field, Some(self.capture))?;
        Ok(())
    }

    pub(crate) fn definition(&self, field: &'static str) -> Result<(), ReadError> {
        if let Some(value) = self.source.field(field) {
            super::definition_shape::read_context(self.source, field, value, Some(self.capture))?;
        }
        Ok(())
    }

    pub(crate) fn definitions(&self, field: &'static str) -> Result<(), ReadError> {
        for value in super::array_context(self.source, field, Some(self.capture))?
            .into_iter()
            .flatten()
        {
            super::definition_shape::read_context(self.source, field, value, Some(self.capture))?;
        }
        Ok(())
    }

    pub(crate) fn property(&self, field: &'static str) -> Result<(), ReadError> {
        if let Some(value) = self.source.field(field) {
            self.property_value(field, value)?;
        }
        Ok(())
    }

    pub(crate) fn properties(&self, field: &'static str) -> Result<(), ReadError> {
        for value in super::array_context(self.source, field, Some(self.capture))?
            .into_iter()
            .flatten()
        {
            self.property_value(field, value)?;
        }
        Ok(())
    }

    fn property_value(&self, field: &'static str, value: &Value<'source>) -> Result<(), ReadError> {
        crate::owned_reference_read::check(value).map_err(|()| {
            super::unexpected_context(
                self.source,
                field,
                value,
                "a nullable weak MissionProperty pointer",
                Some(self.capture),
            )
        })
    }
}
