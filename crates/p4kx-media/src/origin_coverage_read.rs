// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, FieldOrigin};

use crate::MediaError;
use crate::origin_contract::OriginCoverage;

/// Counts orthogonal mapping outcomes without replacing projection health.
pub(crate) struct Coverage {
    fields: u64,
    complete: u64,
    incomplete: u64,
    unknown: u64,
    encrypted: u64,
}

impl Coverage {
    pub(crate) const fn new() -> Self {
        Self {
            fields: 0,
            complete: 0,
            incomplete: 0,
            unknown: 0,
            encrypted: 0,
        }
    }

    pub(crate) fn observe(&mut self, origin: &FieldOrigin) -> Result<(), MediaError> {
        increment(&mut self.fields)?;
        if origin.source_set_complete() {
            increment(&mut self.complete)?;
        } else {
            increment(&mut self.incomplete)?;
        }
        match origin.contains_encrypted_source() {
            Some(true) => increment(&mut self.encrypted)?,
            None => increment(&mut self.unknown)?,
            Some(false) => {}
        }
        Ok(())
    }

    pub(crate) fn finish(&self, expected: &OriginCoverage) -> Result<u64, MediaError> {
        let actual = OriginCoverage {
            fields: DecimalU64::new(self.fields),
            complete: DecimalU64::new(self.complete),
            incomplete: DecimalU64::new(self.incomplete),
            unknown: DecimalU64::new(self.unknown),
            encrypted: DecimalU64::new(self.encrypted),
        };
        if &actual != expected {
            return Err(MediaError::Destination(
                "origin coverage counters disagree with actual fields".into(),
            ));
        }
        Ok(self.fields)
    }
}

fn increment(counter: &mut u64) -> Result<(), MediaError> {
    *counter = counter
        .checked_add(1)
        .ok_or_else(|| MediaError::Destination("origin coverage count overflow".into()))?;
    Ok(())
}
