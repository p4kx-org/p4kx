// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::BankReadError;
use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::read_at::SeekSource;

const HAS_USER_BUSES: u8 = 0x08;
const BUS_ID_LEN: usize = size_of::<u32>();
const USER_BUS_COUNT: usize = 4;
const USER_BUS_BYTES: u64 = (USER_BUS_COUNT * BUS_ID_LEN) as u64;

/// Exact authored user send IDs by slot, with missing words in a truncated list.
pub type BankNodeUserAuxBuses = [Option<BankHircField<u32>>; USER_BUS_COUNT];

/// Checked auxiliary routing declarations in a version-154 node base.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankNodeAuxFields {
    /// Original bank-relative routing position when reached by the shared reader.
    pub start: Option<u64>,
    /// Original Aux flags, including unknown bits.
    pub flags: Option<BankHircField<u8>>,
    /// Checked prefix of full user Aux bus ID words when the list was declared.
    pub user_buses_range: Option<Range<u64>>,
    /// Original reflections Aux bus ID, including zero, when readable.
    pub reflections_bus_id: Option<BankHircField<u32>>,
}

impl BankNodeAuxFields {
    /// Returns the complete original routing extent when the declarations fit.
    #[must_use]
    pub fn complete_range(&self) -> Option<Range<u64>> {
        let start = self.flags.as_ref()?.range.start;
        let end = self.reflections_bus_id.as_ref()?.range.end;
        Some(start..end)
    }
}

pub(crate) fn scan<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankNodeAuxFields,
) -> Result<(), F::Error> {
    observed.start = Some(input.offset());
    let flags = input.read_u8()?;
    let has_buses = flags.value & HAS_USER_BUSES != 0;
    observed.flags = Some(flags);
    if has_buses {
        let start = input.offset();
        let words = input.remaining_bytes().min(USER_BUS_BYTES) / BUS_ID_LEN as u64;
        observed.user_buses_range = Some(start..start + words * BUS_ID_LEN as u64);
        input.skip_bytes(USER_BUS_BYTES)?;
    }
    observed.reflections_bus_id = Some(input.read_u32()?);
    Ok(())
}

pub(crate) fn user_buses<R: Read + Seek>(
    input: &mut SeekSource<R>,
    observed: &BankNodeAuxFields,
) -> Result<Option<BankNodeUserAuxBuses>, BankReadError> {
    let Some(range) = &observed.user_buses_range else {
        return Ok(None);
    };
    let mut ids = [const { None }; USER_BUS_COUNT];
    for (slot, offset) in ids.iter_mut().zip(range.clone().step_by(BUS_ID_LEN)) {
        let mut bytes = [0; BUS_ID_LEN];
        input.read_io_at(offset, &mut bytes)?;
        *slot = Some(BankHircField {
            value: u32::from_le_bytes(bytes),
            range: offset..offset + BUS_ID_LEN as u64,
        });
    }
    Ok(Some(ids))
}
