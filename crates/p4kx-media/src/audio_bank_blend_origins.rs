// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::bank::BankBlendExtent;

use crate::audio_bank_blend_payload::{BlendTable, BlendTableCoverage};
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;
use crate::{MediaError, Status};

#[derive(Clone, Copy)]
pub(crate) struct Occurrence {
    pub(crate) ordinal: u64,
    start: u64,
    end: u64,
}

impl Occurrence {
    pub(crate) fn new(ordinal: u32, range: &Range<u64>) -> Self {
        Self {
            ordinal: u64::from(ordinal),
            start: range.start,
            end: range.end,
        }
    }

    pub(crate) const fn range(self) -> Range<u64> {
        self.start..self.end
    }
}

pub(crate) fn table(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    extent: Option<&BankBlendExtent>,
    name: &str,
    label: &'static str,
) -> Result<Option<BlendTable>, MediaError> {
    let Some(extent) = extent else {
        return Ok(None);
    };
    let count = &extent.declared_count;
    let range = &extent.checked_range;
    sites.field(
        prepared,
        &format!("{name}/declared_count"),
        &count.range,
        label,
    )?;
    sites.range(prepared, &format!("{name}/checked_range"), range)?;
    let coverage = if extent.is_complete() {
        BlendTableCoverage::Complete
    } else {
        BlendTableCoverage::Truncated
    };
    sites.converted(
        prepared,
        &format!("{name}/coverage"),
        &(count.range.start..range.end),
        "blendTableCoverage",
    )?;
    Ok(Some(BlendTable {
        declared_count: count.into(),
        checked_range: range.into(),
        coverage,
    }))
}

pub(crate) fn layout(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    range: &Range<u64>,
    malformed: bool,
) -> Result<Status, MediaError> {
    sites.converted(prepared, "layout", range, "blendRowCoverage")?;
    if malformed {
        sites.issues(prepared, range)?;
        Ok(Status::Partial)
    } else {
        Ok(Status::Complete)
    }
}
