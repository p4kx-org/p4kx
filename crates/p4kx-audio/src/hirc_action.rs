// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_object_fields::{self, BankHircField, ObjectFields};
use crate::bank_reader::SectionReader;
use crate::hirc_node::{PROP_LEN, RANGED_PROP_LEN};
use crate::{BankError, SectionId};

const PLAY_BANK_VERSION: u32 = 154;
const PLAY_ACTION_GROUP: u16 = 0x04;
const PLAY_EVENT_ACTION_GROUP: u16 = 0x21;
const TRIGGER_ACTION_GROUP: u16 = 0x1B;

/// The soundbank a Play action names for its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayBank {
    /// The ID of the soundbank, the hash of its name; 0 for none.
    pub bank_id: u32,
    /// The type of the soundbank reference.
    pub bank_type: u32,
}

/// The action of an Action object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionTarget {
    /// The action type, e.g. `0x0403` for Play.
    pub action_type: u16,
    /// The ID of the object the action is aimed at.
    pub target_id: u32,
    /// The soundbank a Play action names, read in bank version 154 only.
    pub bank: Option<PlayBank>,
}

impl ActionTarget {
    /// Returns whether the action plays its target, action types `0x04xx`.
    #[must_use]
    pub const fn is_play(&self) -> bool {
        self.action_type >> 8 == PLAY_ACTION_GROUP
    }

    /// Returns whether the action posts the Event it is aimed at, action types `0x21xx`.
    #[must_use]
    pub const fn is_play_event(&self) -> bool {
        self.action_type >> 8 == PLAY_EVENT_ACTION_GROUP
    }

    /// Returns whether the action fires the trigger it is aimed at, action types `0x1Bxx`.
    #[must_use]
    pub const fn is_trigger(&self) -> bool {
        self.action_type >> 8 == TRIGGER_ACTION_GROUP
    }
}

/// Original scalar reads used by one Action interpretation.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankActionFields {
    /// Original action type when readable.
    pub action_type: Option<BankHircField<u16>>,
    /// Original action target when readable.
    pub target_id: Option<BankHircField<u32>>,
    /// Original branch decisions for the optional `PlayBank` fields.
    pub play_bank: BankPlayBankDecision,
}

/// The `PlayBank` branch actually reached by the Action reader.
#[derive(Debug, Default)]
#[non_exhaustive]
pub enum BankPlayBankDecision {
    /// Required Action fields failed before the branch could be consulted.
    #[default]
    Unreached,
    /// The bank version has no `PlayBank` fields.
    VersionGated,
    /// The original action type does not identify Play.
    NonPlay,
    /// A Play action in the version with `PlayBank` fields.
    Play(BankPlayBankFields),
}

/// Original reads that locate or supply `PlayBank` fields.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankPlayBankFields {
    /// The count of ordinary properties skipped by this reader, when readable.
    pub property_count: Option<BankHircField<u8>>,
    /// The count of ranged properties skipped by this reader, when readable.
    pub ranged_property_count: Option<BankHircField<u8>>,
    /// The original bank ID, including zero, when readable.
    pub bank_id: Option<BankHircField<u32>>,
    /// The original bank type when readable.
    pub bank_type: Option<BankHircField<u32>>,
}

pub(crate) fn action(
    version: u32,
    fields: &mut SectionReader<'_>,
) -> Result<ActionTarget, BankError> {
    read(version, fields, &mut BankActionFields::default())
}

pub(crate) fn read<F: ObjectFields>(
    version: u32,
    fields: &mut F,
    observed: &mut BankActionFields,
) -> Result<ActionTarget, F::Error> {
    let action_type = fields.read_u16()?;
    let code = action_type.value;
    observed.action_type = Some(action_type);
    let target_id = fields.read_u32()?;
    let mut action = ActionTarget {
        action_type: code,
        target_id: target_id.value,
        bank: None,
    };
    observed.target_id = Some(target_id);

    if version != PLAY_BANK_VERSION {
        observed.play_bank = BankPlayBankDecision::VersionGated;
    } else if action.is_play() {
        let mut play = BankPlayBankFields::default();
        let result = play_bank(fields, &mut play);
        observed.play_bank = BankPlayBankDecision::Play(play);
        action.bank = Some(result?);
    } else {
        observed.play_bank = BankPlayBankDecision::NonPlay;
    }
    Ok(action)
}

fn play_bank<F: ObjectFields>(
    fields: &mut F,
    observed: &mut BankPlayBankFields,
) -> Result<PlayBank, F::Error> {
    fields.read_array::<1>()?;
    let props = fields.read_u8()?;
    let count = u32::from(props.value);
    observed.property_count = Some(props);
    fields.skip_bytes(bank_object_fields::item_bytes(count, PROP_LEN)? as u64)?;

    let ranged = fields.read_u8()?;
    let count = u32::from(ranged.value);
    observed.ranged_property_count = Some(ranged);
    fields.skip_bytes(bank_object_fields::item_bytes(count, RANGED_PROP_LEN)? as u64)?;
    fields.read_array::<1>()?;

    let bank_id = fields.read_u32()?;
    let id = bank_id.value;
    observed.bank_id = Some(bank_id);
    let bank_type = fields.read_u32()?;
    let kind = bank_type.value;
    observed.bank_type = Some(bank_type);
    if fields.remaining_bytes() > 0 {
        return Err(BankError::invalid(
            SectionId::HIRC,
            "bytes follow the fields of a play action",
        )
        .into());
    }
    Ok(PlayBank {
        bank_id: id,
        bank_type: kind,
    })
}
