// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_node_positioning::BankNodePositioningFields;

/// Checked playback parameter groups in the version-154 shared node layout.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankNodePlaybackFields {
    /// Original positioning and automation declarations.
    pub positioning: BankNodePositioningFields,
    /// Original voice and playback behavior settings.
    pub advanced: BankNodeAdvancedFields,
}

/// Original `AdvancedSettings` fields read in physical order.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankNodeAdvancedFields {
    /// Bank-relative start when the region was reached.
    pub start: Option<u64>,
    /// Original voice-limit and virtual-voice flags.
    pub flags: Option<BankHircField<u8>>,
    /// Original virtual queue behavior code.
    pub virtual_queue_behavior: Option<BankHircField<u8>>,
    /// Original instance limit, including zero.
    pub max_instances: Option<BankHircField<u16>>,
    /// Original below-threshold behavior code.
    pub below_threshold_behavior: Option<BankHircField<u8>>,
    /// Original analysis, loudness and envelope flags.
    pub behavior_flags: Option<BankHircField<u8>>,
}

impl BankNodeAdvancedFields {
    /// Returns the exact original settings extent once all fields fit.
    #[must_use]
    pub fn complete_range(&self) -> Option<Range<u64>> {
        Some(self.start?..self.behavior_flags.as_ref()?.range.end)
    }
}

pub(crate) fn advanced<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankNodeAdvancedFields,
) -> Result<(), F::Error> {
    observed.start = Some(input.offset());
    observed.flags = Some(input.read_u8()?);
    observed.virtual_queue_behavior = Some(input.read_u8()?);
    observed.max_instances = Some(input.read_u16()?);
    observed.below_threshold_behavior = Some(input.read_u8()?);
    observed.behavior_flags = Some(input.read_u8()?);
    Ok(())
}
