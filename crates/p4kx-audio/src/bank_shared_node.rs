// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use crate::hirc_node_aux;
use crate::read_at::SeekSource;
use crate::{
    BankInlineStateRows, BankNodeFxKind, BankNodeFxSlots, BankNodeParameters, BankNodeParentFields,
    BankNodePropertyKind, BankNodePropertyRows, BankNodeUserAuxBuses, BankPositioningRows,
    BankPositioningTableKind, BankReadError, BankRtpcRows,
};

/// Checked original `NodeBase` fields with cursors bound to their owning bank.
pub struct BankSharedNode<'input, R> {
    parent: &'input BankNodeParentFields,
    parameters: &'input BankNodeParameters,
    input: &'input mut SeekSource<R>,
}

impl<'input, R> BankSharedNode<'input, R> {
    pub(crate) const fn new(
        parent: &'input BankNodeParentFields,
        parameters: &'input BankNodeParameters,
        input: &'input mut SeekSource<R>,
    ) -> Self {
        Self {
            parent,
            parameters,
            input,
        }
    }

    /// Returns the checked parent, priority and effect declarations.
    #[must_use]
    pub const fn parent(&self) -> &BankNodeParentFields {
        self.parent
    }

    /// Returns the checked groups following the parent declaration.
    #[must_use]
    pub const fn parameters(&self) -> &BankNodeParameters {
        self.parameters
    }

    /// Borrows the checked original effect or metadata slot prefix.
    pub fn fx_slots(&mut self, kind: BankNodeFxKind) -> Option<BankNodeFxSlots<'_, R>> {
        let range = self.parent.fx(kind).fields.slots_range.clone()?;
        Some(BankNodeFxSlots::new(self.input, range))
    }

    /// Reads the checked original user Aux send IDs by authored slot.
    ///
    /// # Errors
    ///
    /// Returns a read error if checked source bytes cannot be read.
    pub fn user_aux_buses(&mut self) -> Result<Option<BankNodeUserAuxBuses>, BankReadError>
    where
        R: Read + Seek,
    {
        hirc_node_aux::user_buses(self.input, &self.parameters.aux)
    }

    /// Borrows checked rows in the selected positioning table.
    pub fn positioning_rows(
        &mut self,
        kind: BankPositioningTableKind,
    ) -> Option<BankPositioningRows<'_, R>> {
        let range = self.parameters.playback.positioning.table(kind)?.clone();
        Some(BankPositioningRows::new(self.input, kind, range))
    }

    /// Borrows the original property list when its count was read.
    pub fn properties(
        &mut self,
        kind: BankNodePropertyKind,
    ) -> Option<BankNodePropertyRows<'_, R>> {
        self.parameters
            .properties
            .get(kind)
            .map(|extent| BankNodePropertyRows::new(self.input, extent))
    }

    /// Borrows the original State chunk when its whole extent was checked.
    pub fn states(&mut self) -> Option<BankInlineStateRows<'_, R>> {
        self.parameters
            .state_chunk
            .as_ref()
            .map(|range| BankInlineStateRows::new(self.input, range.clone()))
    }

    /// Borrows the original RTPC count and complete curve prefix.
    pub fn rtpc(&mut self) -> Option<BankRtpcRows<'_, R>> {
        self.parameters
            .rtpc_chunk
            .as_ref()
            .map(|extent| BankRtpcRows::new(self.input, extent))
    }
}
