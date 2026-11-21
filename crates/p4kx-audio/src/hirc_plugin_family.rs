// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::HircKind;
use crate::bank_object_fields::ObjectFields;
use crate::hirc_device::{self, BankDeviceSlots};
use crate::hirc_effect::{self, BankEffectFields};

pub(crate) const VERSION: u32 = 154;

/// The physical kind of an effect-base plug-in owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BankPluginKind {
    /// Authored effect share set.
    ShareSet,
    /// Authored custom effect.
    Custom,
    /// Audio device with its own effect slots.
    AudioDevice,
}

impl BankPluginKind {
    /// Returns a plug-in owner kind when the physical HIRC type is recognized.
    #[must_use]
    pub const fn of(kind: HircKind) -> Option<Self> {
        match kind {
            HircKind::FxShareSet => Some(Self::ShareSet),
            HircKind::FxCustom => Some(Self::Custom),
            HircKind::AudioDevice => Some(Self::AudioDevice),
            _ => None,
        }
    }

    pub(crate) fn device_slots(self) -> Option<BankDeviceSlots> {
        (self == Self::AudioDevice).then(BankDeviceSlots::default)
    }
}

pub(crate) fn scan<F: ObjectFields>(
    input: &mut F,
    effect: &mut BankEffectFields,
    slots: Option<&mut BankDeviceSlots>,
) -> Result<(), F::Error> {
    match slots {
        Some(slots) => hirc_device::scan(input, effect, slots)?,
        None => hirc_effect::scan(input, effect)?,
    }
    input.require_exhausted("bytes follow the plug-in object")
}
