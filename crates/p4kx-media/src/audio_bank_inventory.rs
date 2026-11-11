// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SectionId;
use p4kx_extraction::bank::{BankObservedHeader, BankSourceSection};

use crate::MediaError;

pub(crate) struct BankInventory {
    pub(crate) last_child_section: Option<u64>,
    pub(crate) complete: bool,
    headers: u64,
    header: Option<BankObservedHeader>,
    data: DataInventory,
}

#[derive(Clone, Copy)]
pub(crate) struct VersionSite {
    pub(crate) version: u32,
    pub(crate) body_offset: u64,
}

#[derive(Default)]
pub(crate) enum DataInventory {
    #[default]
    Missing,
    Unique(BankSourceSection),
    Ambiguous(u64),
}

impl BankInventory {
    pub(crate) const fn new() -> Self {
        Self {
            last_child_section: None,
            complete: true,
            headers: 0,
            header: None,
            data: DataInventory::Missing,
        }
    }

    pub(crate) const fn seen_section(&mut self, section: &BankSourceSection) {
        if matches!(
            section.id(),
            SectionId::DIDX | SectionId::HIRC | SectionId::STID
        ) {
            self.last_child_section = Some(section.ordinal());
        }
    }

    pub(crate) fn header(&mut self, header: Option<BankObservedHeader>) -> Result<(), MediaError> {
        self.headers = self
            .headers
            .checked_add(1)
            .ok_or_else(crate::origin_error::counter)?;
        self.header = if self.headers == 1 {
            header.filter(|header| header.header().is_supported_version())
        } else {
            None
        };
        Ok(())
    }

    pub(crate) fn observe_data(&mut self, section: BankSourceSection) -> Result<(), MediaError> {
        self.data = match self.data {
            DataInventory::Missing => DataInventory::Unique(section),
            DataInventory::Unique(_) => DataInventory::Ambiguous(2),
            DataInventory::Ambiguous(count) => DataInventory::Ambiguous(
                count
                    .checked_add(1)
                    .ok_or_else(crate::origin_error::counter)?,
            ),
        };
        Ok(())
    }

    pub(crate) const fn observed_header(&self) -> Option<&BankObservedHeader> {
        if self.complete {
            self.header.as_ref()
        } else {
            None
        }
    }

    pub(crate) fn data(&self) -> Option<&DataInventory> {
        self.complete.then_some(&self.data)
    }
}
