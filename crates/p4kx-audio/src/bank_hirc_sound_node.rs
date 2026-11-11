// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_node_property_rows::BankNodePropertyRows;
use crate::bank_source_sound::BankHircSound;
use crate::hirc_node_property::BankNodePropertyKind;
use crate::read_at::SeekSource;

use super::sound_rtpc::{self, BankRtpcRead};
use super::sound_states::{self, BankSoundStateRead};

/// Original Sound semantics and source-bound cursors for its node chunks.
pub struct BankHircSoundNode<'input, R> {
    input: &'input mut SeekSource<R>,
    sound: Box<BankHircSound>,
}

impl<'input, R> BankHircSoundNode<'input, R> {
    pub(super) const fn new(input: &'input mut SeekSource<R>, sound: Box<BankHircSound>) -> Self {
        Self { input, sound }
    }

    /// Returns the original Sound observation.
    #[must_use]
    pub const fn sound(&self) -> &BankHircSound {
        &self.sound
    }

    /// Borrows the original property list when its count was read.
    pub fn properties(
        &mut self,
        kind: BankNodePropertyKind,
    ) -> Option<BankNodePropertyRows<'_, R>> {
        self.sound
            .fields
            .node
            .as_ref()
            .and_then(|node| node.base.as_ref())
            .and_then(|base| base.properties.get(kind))
            .map(|extent| BankNodePropertyRows::new(self.input, extent))
    }

    /// Borrows the original checked inline State region.
    pub fn states(&mut self) -> BankSoundStateRead<'_, R> {
        sound_states::read(self.input, &self.sound)
    }

    /// Borrows the original checked RTPC curve prefix.
    pub fn rtpc(&mut self) -> BankRtpcRead<'_, R> {
        sound_rtpc::read(self.input, &self.sound)
    }
}
