// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(super) struct ParticleResource {
    path: SourcedValue<String>,
}

impl ParticleResource {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            path: source.string("path", notes)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct AudioResource {
    audio_trigger: SourcedValue<String>,
}

impl AudioResource {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            audio_trigger: source.string("audioTrigger", notes)?,
        })
    }

    pub(super) const fn projected_member(
        &self,
    ) -> (&'static str, p4kx_extraction::SourceValueState) {
        ("audioTrigger", self.audio_trigger.state())
    }
}

/// The authored audio RTPC identifier.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// An authored real-time parameter control identifier.
pub struct Rtpc {
    rtpc: SourcedValue<String>,
}

impl Rtpc {
    /// Borrows the original RTPC identifier.
    #[must_use]
    pub const fn rtpc(&self) -> &SourcedValue<String> {
        &self.rtpc
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            rtpc: source.string("rtpc", notes)?,
        })
    }

    pub(super) const fn projected_member(
        &self,
    ) -> (&'static str, p4kx_extraction::SourceValueState) {
        ("rtpc", self.rtpc.state())
    }
}
