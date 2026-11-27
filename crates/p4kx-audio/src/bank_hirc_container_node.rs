// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use crate::bank_blend_rows::BankBlendLayers;
use crate::bank_inline_state_rows::BankInlineStateRows;
use crate::bank_node_fx_slots::BankNodeFxSlots;
use crate::bank_node_property_rows::BankNodePropertyRows;
use crate::bank_positioning_rows::BankPositioningRows;
use crate::bank_random_sequence_rows::BankRandomSequenceRows;
use crate::bank_rtpc_rows::BankRtpcRows;
use crate::bank_switch_configuration_rows::BankSwitchConfigurationRows;
use crate::bank_switch_group_rows::BankSwitchGroupRows;
use crate::hirc_container::{BankContainerFields, Layout};
use crate::hirc_node_aux::{self, BankNodeUserAuxBuses};
use crate::hirc_node_fx::BankNodeFxKind;
use crate::hirc_node_positioning::BankPositioningTableKind;
use crate::hirc_node_property::BankNodePropertyKind;
use crate::read_at::SeekSource;
use crate::{BankHircObject, BankReadError, HircKind};

use super::{BankContainerChildren, BankContainerRead, Observation, child_reader, observed};

/// A container's original interpretation and sequential cursors bound to its bank.
pub struct BankHircContainerNode<'input, R> {
    object: BankHircObject,
    observation: Observation,
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
}

impl<R> BankHircContainerNode<'_, R> {
    /// Returns the original physical container.
    #[must_use]
    pub const fn object(&self) -> &BankHircObject {
        &self.object
    }

    /// Returns the original container body disposition.
    #[must_use]
    pub const fn status(&self) -> &BankContainerRead {
        &self.observation.status
    }

    /// Returns original container fields and checked node extents.
    #[must_use]
    pub const fn fields(&self) -> &BankContainerFields {
        &self.observation.fields
    }

    /// Borrows the original child list when the declared list fits.
    pub fn children(&mut self) -> Option<BankContainerChildren<'_, R>> {
        let fields = &self.observation.fields;
        let (range, count) = fields.children.as_ref().zip(fields.child_count.as_ref())?;
        Some(child_reader(self.input, self.stopped, range, count.value))
    }

    /// Borrows checked Random/Sequence playlist items in physical order.
    pub fn playlist(&mut self) -> Option<BankRandomSequenceRows<'_, R>> {
        let playlist = self
            .observation
            .fields
            .random_sequence
            .as_ref()?
            .playlist
            .as_ref()?;
        Some(BankRandomSequenceRows::new(
            self.input,
            playlist.checked_range.clone(),
        ))
    }

    /// Borrows checked original Switch membership groups and members.
    pub fn switch_groups(&mut self) -> Option<BankSwitchGroupRows<'_, R>> {
        let extent = self.observation.fields.switch.as_ref()?.groups.as_ref()?;
        Some(BankSwitchGroupRows::new(self.input, self.stopped, extent))
    }

    /// Borrows checked original per-child Switch configurations.
    pub fn switch_configurations(&mut self) -> Option<BankSwitchConfigurationRows<'_, R>> {
        let extent = self
            .observation
            .fields
            .switch
            .as_ref()?
            .configurations
            .as_ref()?;
        Some(BankSwitchConfigurationRows::new(
            self.input,
            self.stopped,
            extent,
        ))
    }

    /// Borrows checked original Blend Layers with nested source cursors.
    pub fn blend_layers(&mut self) -> Option<BankBlendLayers<'_, R>> {
        let extent = self.observation.fields.blend.as_ref()?.layers.as_ref()?;
        Some(BankBlendLayers::new(self.input, self.stopped, extent))
    }

    /// Borrows the checked original property rows.
    pub fn properties(
        &mut self,
        kind: BankNodePropertyKind,
    ) -> Option<BankNodePropertyRows<'_, R>> {
        let extent = self.observation.fields.parameters.properties.get(kind)?;
        Some(BankNodePropertyRows::new(self.input, extent))
    }

    /// Borrows the checked original effect or metadata slots.
    pub fn fx_slots(&mut self, kind: BankNodeFxKind) -> Option<BankNodeFxSlots<'_, R>> {
        let range = self
            .observation
            .fields
            .node
            .fx(kind)
            .fields
            .slots_range
            .clone()?;
        Some(BankNodeFxSlots::new(self.input, range))
    }

    /// Reads the checked original user Aux bus IDs by authored slot.
    ///
    /// # Errors
    ///
    /// Returns a read error if checked source bytes cannot be read.
    pub fn user_aux_buses(&mut self) -> Result<Option<BankNodeUserAuxBuses>, BankReadError>
    where
        R: Read + Seek,
    {
        hirc_node_aux::user_buses(self.input, &self.observation.fields.parameters.aux)
    }

    /// Borrows checked rows in the selected positioning table.
    pub fn positioning_rows(
        &mut self,
        kind: BankPositioningTableKind,
    ) -> Option<BankPositioningRows<'_, R>> {
        let range = self
            .observation
            .fields
            .parameters
            .playback
            .positioning
            .table(kind)?
            .clone();
        Some(BankPositioningRows::new(self.input, kind, range))
    }

    /// Borrows the complete checked inline State region.
    pub fn states(&mut self) -> Option<BankInlineStateRows<'_, R>> {
        let range = self.observation.fields.parameters.state_chunk.clone()?;
        Some(BankInlineStateRows::new(self.input, range))
    }

    /// Borrows the original RTPC count and checked curve prefix.
    pub fn rtpc(&mut self) -> Option<BankRtpcRows<'_, R>> {
        let extent = self.observation.fields.parameters.rtpc_chunk.as_ref()?;
        Some(BankRtpcRows::new(self.input, extent))
    }
}

pub(crate) fn observe_node<'input, R: Read + Seek>(
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    object: BankHircObject,
    version: u32,
    kind: HircKind,
    layout: Layout,
) -> Result<BankHircContainerNode<'input, R>, BankReadError> {
    let observation = observed(input, stopped, &object, version, kind, layout)?;
    Ok(BankHircContainerNode {
        object,
        observation,
        input,
        stopped,
    })
}
