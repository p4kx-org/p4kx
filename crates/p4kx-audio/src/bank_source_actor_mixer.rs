// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use crate::bank_inline_state_rows::BankInlineStateRows;
use crate::bank_node_fx_slots::BankNodeFxSlots;
use crate::bank_node_property_rows::BankNodePropertyRows;
use crate::bank_object_fields::{ObjectFields, SeekObjectFields};
use crate::bank_positioning_rows::BankPositioningRows;
use crate::bank_rtpc_rows::BankRtpcRows;
use crate::bank_source_container::{self, BankContainerChildren, BankContainerRead};
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc_actor_mixer::{self, BankActorMixerFields};
use crate::hirc_node_aux::{self, BankNodeUserAuxBuses};
use crate::hirc_node_fx::BankNodeFxKind;
use crate::hirc_node_positioning::BankPositioningTableKind;
use crate::hirc_node_property::BankNodePropertyKind;
use crate::read_at::SeekSource;
use crate::{BankError, BankHircObject, BankReadError, HircKind};

/// Original `ActorMixer` semantics and sequential cursors bound to the owning bank.
pub struct BankHircActorMixer<'input, R> {
    object: BankHircObject,
    status: BankContainerRead,
    fields: BankActorMixerFields,
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
}

impl<R> BankHircActorMixer<'_, R> {
    /// Returns the original physical object.
    #[must_use]
    pub const fn object(&self) -> &BankHircObject {
        &self.object
    }

    /// Returns the layout disposition of the original body.
    #[must_use]
    pub const fn status(&self) -> &BankContainerRead {
        &self.status
    }

    /// Returns original scalar fields and checked extents.
    #[must_use]
    pub const fn fields(&self) -> &BankActorMixerFields {
        &self.fields
    }

    /// Borrows the checked original effect or metadata slot prefix.
    pub fn fx_slots(&mut self, kind: BankNodeFxKind) -> Option<BankNodeFxSlots<'_, R>> {
        let range = self.fields.node.fx(kind).fields.slots_range.clone()?;
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
        hirc_node_aux::user_buses(self.input, &self.fields.aux)
    }

    /// Borrows checked rows in the selected positioning table.
    pub fn positioning_rows(
        &mut self,
        kind: BankPositioningTableKind,
    ) -> Option<BankPositioningRows<'_, R>> {
        let range = self.fields.playback.positioning.table(kind)?.clone();
        Some(BankPositioningRows::new(self.input, kind, range))
    }

    /// Borrows the original property list when its count was read.
    pub fn properties(
        &mut self,
        kind: BankNodePropertyKind,
    ) -> Option<BankNodePropertyRows<'_, R>> {
        self.fields
            .properties
            .get(kind)
            .map(|extent| BankNodePropertyRows::new(self.input, extent))
    }

    /// Borrows the original State chunk when its whole extent was checked.
    pub fn states(&mut self) -> Option<BankInlineStateRows<'_, R>> {
        self.fields
            .state_chunk
            .as_ref()
            .map(|range| BankInlineStateRows::new(self.input, range.clone()))
    }

    /// Borrows the original RTPC count and complete curve prefix when available.
    pub fn rtpc(&mut self) -> Option<BankRtpcRows<'_, R>> {
        self.fields
            .rtpc_chunk
            .as_ref()
            .map(|extent| BankRtpcRows::new(self.input, extent))
    }

    /// Borrows complete child words in authored order, even for a truncated list.
    pub fn children(&mut self) -> Option<BankContainerChildren<'_, R>> {
        self.fields.children.as_ref().map(|range| {
            bank_source_container::child_reader(
                self.input,
                self.stopped,
                range,
                self.fields.checked_children,
            )
        })
    }
}

pub(crate) fn observe<'input, R: Read + Seek>(
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    object: BankHircObject,
    version: u32,
) -> Result<BankHircActorMixer<'input, R>, BankReadError> {
    let mut fields = BankActorMixerFields::default();
    let status = if version == hirc_actor_mixer::VERSION {
        match scan(input, &object, &mut fields) {
            Ok(()) => BankContainerRead::Decoded,
            Err(BankReadError::Format(error)) => BankContainerRead::Malformed(error),
            Err(error) => {
                *stopped = true;
                return Err(error);
            }
        }
    } else {
        BankContainerRead::Unsupported(BankError::UnsupportedObjectLayout {
            kind: HircKind::ActorMixer,
            version,
        })
    };
    Ok(BankHircActorMixer {
        object,
        status,
        fields,
        input,
        stopped,
    })
}

fn scan<R: Read + Seek>(
    input: &mut SeekSource<R>,
    object: &BankHircObject,
    observed: &mut BankActorMixerFields,
) -> Result<(), BankReadError> {
    let mut fields = SeekObjectFields::after_id(
        input,
        object.body_offset(),
        object.body_len(),
        OBJECT_ID_LEN as u64,
    );
    let header = hirc_actor_mixer::head(&mut fields, observed)?;
    fields.skip_bytes(header.child_bytes)?;
    hirc_actor_mixer::finish(&fields, observed)
}
