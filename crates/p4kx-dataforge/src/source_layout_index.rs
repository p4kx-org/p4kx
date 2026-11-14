// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::dataforge;
use crate::layout_cursor::LayoutCursor;
use crate::rows::StructDef;
use crate::source_layout_allocation;
use crate::source_layout_properties::{SourceField, SourceProperties};
use crate::source_layout_validation::{self, LayoutFailure, LayoutSummary};
use crate::{DataForge, DataForgeError};

/// Immutable physical declarations and flat validated inheritance summaries.
#[derive(Debug)]
pub(crate) struct SourceLayoutIndex {
    pub(crate) properties: SourceProperties,
    rows: Box<[StructDef]>,
    summaries: Box<[Result<LayoutSummary, LayoutFailure>]>,
}

/// One structurally validated layout, independent of its optional type-name string.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceLayout<'source> {
    pub(crate) index: &'source SourceLayoutIndex,
    pub(crate) forge: &'source DataForge<'source>,
    pub(crate) summary: &'source LayoutSummary,
}

impl SourceLayoutIndex {
    pub(crate) fn read(forge: &DataForge<'_>) -> Result<Self, DataForgeError> {
        let mut rows = Vec::new();
        source_layout_allocation::reserve(&mut rows, forge.structs.len(), "source layout rows")?;
        rows.extend(forge.structs.iter());
        let properties = SourceProperties::read(forge, &rows)?;
        let summaries = source_layout_validation::validate(&rows, &properties)?;
        Ok(Self {
            properties,
            rows: rows.into_boxed_slice(),
            summaries,
        })
    }

    pub(crate) fn layout<'source>(
        &'source self,
        forge: &'source DataForge<'source>,
        struct_index: u32,
    ) -> Result<SourceLayout<'source>, DataForgeError> {
        if !std::ptr::eq(self, forge.source_layouts()?) {
            return Err(DataForgeError::LayoutInvariant);
        }
        let index = usize::try_from(struct_index).map_err(|_| DataForgeError::LayoutInvariant)?;
        let summary = self.summary(index, struct_index)?;
        Ok(SourceLayout {
            index: self,
            forge,
            summary,
        })
    }

    pub(crate) fn summary(
        &self,
        index: usize,
        requested: u32,
    ) -> Result<&LayoutSummary, DataForgeError> {
        match self.summaries.get(index) {
            Some(Ok(summary)) => Ok(summary),
            Some(Err(LayoutFailure::Cycle)) => Err(DataForgeError::StructCycle {
                struct_index: requested,
            }),
            Some(Err(LayoutFailure::Source(error))) => Err(error.clone()),
            None => Err(dataforge::out_of_range(
                "struct",
                requested as usize,
                "struct",
                index as u64,
            )),
        }
    }

    pub(crate) fn row(&self, index: usize) -> Result<StructDef, DataForgeError> {
        self.rows
            .get(index)
            .copied()
            .ok_or(DataForgeError::LayoutInvariant)
    }

    pub(crate) fn physical_fields(&self) -> &[SourceField] {
        &self.properties.fields
    }
}

impl<'source> SourceLayout<'source> {
    pub(crate) const fn field_count(&self) -> u64 {
        self.summary.field_count
    }

    pub(crate) fn cursor(&self) -> Result<LayoutCursor<'source>, DataForgeError> {
        LayoutCursor::new(*self)
    }
}

impl DataForge<'_> {
    pub(crate) fn source_layouts(&self) -> Result<&SourceLayoutIndex, DataForgeError> {
        if let Some(index) = self.source_layout_index.get() {
            return Ok(index);
        }
        let _turn = self
            .source_layout_turn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(index) = self.source_layout_index.get() {
            return Ok(index);
        }
        let index = SourceLayoutIndex::read(self)?;
        self.source_layout_index
            .set(index)
            .map_err(|_| DataForgeError::LayoutInvariant)?;
        self.source_layout_index
            .get()
            .ok_or(DataForgeError::LayoutInvariant)
    }
}

/// Returns whether a layout failure describes an operation rather than malformed source data.
pub(crate) const fn is_operational(error: &DataForgeError) -> bool {
    matches!(
        error,
        DataForgeError::LayoutAllocationRefused { .. }
            | DataForgeError::LayoutInvariant
            | DataForgeError::MaterializationBudgetExceeded { .. }
            | DataForgeError::MaterializationAllocationRefused { .. }
            | DataForgeError::CanonicalTreeInvariant
            | DataForgeError::ObservationStopped
    )
}
