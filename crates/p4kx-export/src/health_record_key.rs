// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::HealthText;

/// Value-only identity for physical accounting and deterministic diagnostic samples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordSubjectKey {
    index: u32,
    guid: Guid,
    path: Option<HealthText>,
}

impl RecordSubjectKey {
    pub(super) const fn new(index: u32, guid: Guid, path: Option<HealthText>) -> Self {
        Self { index, guid, path }
    }

    pub(crate) const fn index(&self) -> u32 {
        self.index
    }

    pub(crate) const fn guid(&self) -> Guid {
        self.guid
    }

    pub(crate) fn path(&self) -> Option<&str> {
        self.path.as_ref().map(HealthText::as_str)
    }

    pub(crate) const fn path_text(&self) -> Option<&HealthText> {
        self.path.as_ref()
    }
    pub(crate) fn sample_guid(&self) -> [u8; 36] {
        crate::guid_text::CatalogGuid(self.guid).text()
    }
}
