// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_node_property_rows::BankNodePropertyRows;
use crate::bank_object_fields::SeekObjectFields;
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc_dialogue_event::{self, BankDialogueEventFields};
use crate::hirc_music_switch::SwitchGroups;
use crate::read_at::SeekSource;
use crate::{BankError, BankHircObject, BankNodePropertyKind, BankReadError};

#[path = "bank_source_dialogue_event_rows.rs"]
mod rows;

pub use self::rows::{BankDialogueEventRow, BankDialogueEventRows, BankDialogueNode};

/// A `DialogueEvent` and the owner-bound replay of its checked original fields.
pub struct BankHircDialogueEvent<'input, R> {
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    object: BankHircObject,
    fields: BankDialogueEventFields,
    arguments: SwitchGroups,
    error: Option<BankError>,
}

impl<R> BankHircDialogueEvent<'_, R> {
    /// Returns the original physical HIRC envelope.
    #[must_use]
    pub const fn object(&self) -> &BankHircObject {
        &self.object
    }

    /// Returns successfully observed source fields and list extents.
    #[must_use]
    pub const fn fields(&self) -> &BankDialogueEventFields {
        &self.fields
    }

    /// Returns a format error when the declared original layout is incomplete.
    #[must_use]
    pub const fn format_error(&self) -> Option<&BankError> {
        self.error.as_ref()
    }

    /// Returns the number of complete original argument IDs.
    #[must_use]
    pub const fn checked_arguments(&self) -> u32 {
        self.arguments.ids.count
    }

    /// Returns the checked original argument ID extent when its count was observed.
    #[must_use]
    pub fn argument_ids_range(&self) -> Option<Range<u64>> {
        self.fields.argument_count.as_ref()?;
        let ids = self.arguments.ids;
        Some(ids.start..ids.start + ids.length)
    }

    /// Returns the checked type extent and count once the full ID table fits.
    #[must_use]
    pub fn argument_types(&self) -> Option<(Range<u64>, u32)> {
        let count = self.fields.argument_count.as_ref()?;
        if count.value != self.arguments.ids.count {
            return None;
        }
        let types = self.arguments.types;
        Some((types.start..types.start + types.length, types.count))
    }

    /// Borrows the original checked argument and decision node rows in source order.
    pub const fn rows(&mut self) -> BankDialogueEventRows<'_, R> {
        BankDialogueEventRows::new(
            self.input,
            self.stopped,
            &self.arguments,
            self.fields.tree.as_ref(),
        )
    }

    /// Borrows one authored property list when its count was observed.
    pub fn properties(
        &mut self,
        kind: BankNodePropertyKind,
    ) -> Option<BankNodePropertyRows<'_, R>> {
        let extent = self.fields.properties.get(kind)?;
        Some(BankNodePropertyRows::new(self.input, extent))
    }
}

pub(crate) fn observe<'input, R: Read + Seek>(
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    object: BankHircObject,
) -> Result<BankHircDialogueEvent<'input, R>, BankReadError> {
    let mut reader = SeekObjectFields::after_id(
        input,
        object.body_offset(),
        object.body_len(),
        OBJECT_ID_LEN as u64,
    );
    let mut fields = BankDialogueEventFields::default();
    let mut arguments = SwitchGroups::default();
    let error = match hirc_dialogue_event::scan(&mut reader, &mut fields, &mut arguments) {
        Ok(()) => None,
        Err(BankReadError::Format(error)) => Some(error),
        Err(error) => {
            *stopped = true;
            return Err(error);
        }
    };
    Ok(BankHircDialogueEvent {
        input,
        stopped,
        object,
        fields,
        arguments,
        error,
    })
}
