// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::io;

use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Report {
    pub(in super::super) decoded_bytes: u64,
    pub(super) framing_bytes: u64,
    pub(in super::super) physical_sources: u64,
    pub(in super::super) source_uses: u64,
    pub(in super::super) sections: Vec<Section>,
    pub(super) reference_sets: ReferenceSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct Section {
    pub(super) group: Option<String>,
    pub(super) name: String,
    pub(super) value_bytes: u64,
    pub(super) member_bytes: u64,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReferenceSummary {
    pub(super) arrays: u64,
    pub(super) empty_arrays: u64,
    pub(super) references: u64,
    pub(super) array_bytes: u64,
    pub(super) distinct_nonempty_sha256_fingerprints: usize,
    pub(super) repeated_nonempty_sha256_fingerprints: u64,
    pub(super) nonempty_arrays_repeating_a_sha256_fingerprint: u64,
}

#[derive(Default)]
pub(super) struct ReferenceSets {
    active: Option<Active>,
    measured: ReferenceSummary,
    frequencies: HashMap<[u8; 32], u64>,
}

struct Active {
    start: u64,
    items: u64,
    digest: Sha256,
}

impl ReferenceSets {
    pub(super) const fn active(&self) -> bool {
        self.active.is_some()
    }

    pub(super) fn begin(&mut self, start: u64) -> io::Result<()> {
        if self.active.is_some() {
            return Err(invalid("overlapping reference arrays"));
        }
        self.active = Some(Active {
            start,
            items: 0,
            digest: Sha256::new(),
        });
        Ok(())
    }

    pub(super) fn item(&mut self) -> io::Result<()> {
        let active = self
            .active
            .as_mut()
            .ok_or_else(|| invalid("unbound source reference"))?;
        add(&mut active.items, 1)
    }

    pub(super) fn hash(&mut self, bytes: &[u8]) -> io::Result<()> {
        let active = self
            .active
            .as_mut()
            .ok_or_else(|| invalid("unbound source reference bytes"))?;
        active.digest.update(bytes);
        Ok(())
    }

    pub(super) fn end(&mut self, at: u64) -> io::Result<()> {
        let active = self
            .active
            .take()
            .ok_or_else(|| invalid("unbound source reference end"))?;
        add(&mut self.measured.arrays, 1)?;
        add(&mut self.measured.references, active.items)?;
        add(&mut self.measured.array_bytes, at - active.start)?;
        if active.items == 0 {
            return add(&mut self.measured.empty_arrays, 1);
        }
        let frequency = self
            .frequencies
            .entry(active.digest.finalize().into())
            .or_default();
        if *frequency == 1 {
            add(&mut self.measured.repeated_nonempty_sha256_fingerprints, 1)?;
        }
        if *frequency > 0 {
            add(
                &mut self.measured.nonempty_arrays_repeating_a_sha256_fingerprint,
                1,
            )?;
        }
        add(frequency, 1)
    }

    pub(super) fn report(self) -> ReferenceSummary {
        ReferenceSummary {
            distinct_nonempty_sha256_fingerprints: self.frequencies.len(),
            ..self.measured
        }
    }
}

fn add(counter: &mut u64, amount: u64) -> io::Result<()> {
    *counter = counter
        .checked_add(amount)
        .ok_or_else(|| invalid("reference counter overflow"))?;
    Ok(())
}

pub(super) fn start_inventory(count: &mut Option<u64>) -> io::Result<()> {
    if count.replace(0).is_some() {
        return Err(invalid("duplicate inventory section"));
    }
    Ok(())
}

pub(super) fn increment(count: &mut Option<u64>) -> io::Result<()> {
    let count = count
        .as_mut()
        .ok_or_else(|| invalid("missing inventory accumulator"))?;
    *count = count
        .checked_add(1)
        .ok_or_else(|| invalid("row count overflow"))?;
    Ok(())
}

pub(super) fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
