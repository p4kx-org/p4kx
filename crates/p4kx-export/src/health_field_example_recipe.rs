// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Guid, SourceValue};

use super::super::health_example_description::ExampleDescription;
use super::super::health_reference_path::ReferencePathReceipt;
use super::super::health_reference_receipt::{ReferenceDatum, ReferenceFrame};
use super::ExampleBody;

/// Retains a checked field path and its shared original operands until report emission.
#[derive(Debug, Clone)]
pub(super) struct FieldExampleRecipe {
    frame: Arc<ReferenceFrame>,
    path: ReferencePathReceipt,
}

impl FieldExampleRecipe {
    pub(super) const fn new(frame: Arc<ReferenceFrame>, path: ReferencePathReceipt) -> Self {
        Self { frame, path }
    }

    pub(super) fn frame(&self) -> &ReferenceFrame {
        &self.frame
    }

    pub(super) fn description(&self) -> ExampleDescription {
        (
            Some(self.path.field.clone()),
            Some(self.path.owner.type_name.clone()),
            None,
        )
    }

    pub(super) fn matches(
        &self,
        field: Option<&str>,
        source_type: Option<&str>,
        target_guid: Option<Guid>,
    ) -> bool {
        field == Some(self.path.field.as_str())
            && source_type == Some(self.path.owner.type_name.as_str())
            && target_guid.is_none()
    }

    pub(super) fn into_body(self) -> ExampleBody {
        ExampleBody::Captured {
            frame: self.frame,
            field: ReferenceDatum::captured(
                SourceValue::Present(self.path.field),
                self.path.uses.into_vec(),
            ),
            source_type: ReferenceDatum::captured(
                SourceValue::Present(self.path.owner.type_name),
                vec![self.path.owner.input],
            ),
            target_guid: ReferenceDatum::absent(),
        }
    }
}
