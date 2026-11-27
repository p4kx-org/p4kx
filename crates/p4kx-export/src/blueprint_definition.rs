// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::PhysicalSourceRef;
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceArenaOutput;

/// The original fields and independently sourced header of one selected Blueprint Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintDefinition<'source> {
    #[serde(skip)]
    pub(crate) physical_guid: CatalogGuid,
    #[serde(skip)]
    pub(crate) physical_path: &'source str,
    #[serde(skip)]
    pub(crate) physical_index: u32,
    #[serde(skip)]
    pub(crate) numbers: Vec<CapturedNumber>,
    pub(crate) input_ref: PhysicalSourceRef,
    #[serde(flatten)]
    pub(crate) header: CanonicalRecordHeader<'source>,
    pub(crate) source: SourceArenaOutput<'source>,
}

/// Structural navigation to one emitted physical Blueprint Record.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct PhysicalBlueprintRef {
    pub(crate) input_ref: PhysicalSourceRef,
    pub(crate) record_index: u32,
}

impl<'source> BlueprintDefinition<'source> {
    /// Borrows the physical input containing this Blueprint.
    #[must_use]
    pub const fn input_ref(&self) -> &PhysicalSourceRef {
        &self.input_ref
    }

    /// Borrows the independently sourced physical Record header.
    #[must_use]
    pub const fn header(&self) -> &CanonicalRecordHeader<'source> {
        &self.header
    }

    /// Borrows the complete owner-local original value arena.
    #[must_use]
    pub const fn source(&self) -> &SourceArenaOutput<'source> {
        &self.source
    }
}

impl PhysicalBlueprintRef {
    /// Borrows the physical input containing the target Blueprint.
    #[must_use]
    pub const fn input_ref(&self) -> &PhysicalSourceRef {
        &self.input_ref
    }

    /// Returns the physical Record position within that input.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.record_index
    }
}
