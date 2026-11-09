// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::ObservedEntryBinding;

#[derive(Debug, Clone, Copy)]
pub(crate) enum SourceNonce {
    Known,
    Legacy(usize),
    View(usize),
}

impl SourceNonce {
    pub(crate) fn key(self) -> String {
        match self {
            Self::Known => String::new(),
            Self::Legacy(slot) => format!("x{slot}"),
            Self::View(slot) => format!("v{slot}"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ArchiveSourceBinding {
    pub(crate) original: ObservedEntryBinding,
    pub(crate) nonce: SourceNonce,
}

impl ArchiveSourceBinding {
    pub(crate) fn matches(&self, original: &ObservedEntryBinding) -> bool {
        self.original.is_same_entry(original)
    }
}
