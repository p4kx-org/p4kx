// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_effect::{self, BankEffectFields};
use crate::hirc_node_fx::{self, BankNodeFxFields, BankNodeFxKind, BankNodeFxList};

/// Original fields specific to the authored effect slots of a v154 audio device.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankDeviceSlots {
    /// Checked optional bypass and physical slot rows.
    pub fields: BankNodeFxFields,
    /// Original number of slot rows.
    pub count: Option<BankHircField<u8>>,
}

impl BankDeviceSlots {
    /// Returns the checked device list when its first byte was reached.
    #[must_use]
    pub const fn list(&self) -> BankNodeFxList<'_> {
        BankNodeFxList {
            declared_count: self.count.as_ref(),
            fields: &self.fields,
        }
    }

    pub(crate) fn checked_end(&self) -> Option<u64> {
        self.fields
            .slots_range
            .as_ref()
            .map(|range| range.end)
            .or_else(|| self.count.as_ref().map(|field| field.range.end))
    }
}

pub(crate) fn scan<F: ObjectFields>(
    input: &mut F,
    effect: &mut BankEffectFields,
    slots: &mut BankDeviceSlots,
) -> Result<(), F::Error> {
    hirc_effect::scan(input, effect)?;
    hirc_node_fx::scan_owned(
        input,
        BankNodeFxKind::Effects,
        &mut slots.fields,
        &mut slots.count,
    )
}
