// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{LoadoutFileObservation, LoadoutXmlDecoding};

/// An attribute borrowed from the actual parse of its original file operation.
#[derive(Debug)]
pub struct XmlLoadoutAttributeSite<'parse> {
    pub(super) observation: &'parse LoadoutFileObservation,
    pub(super) node_index: u32,
    pub(super) attribute_index: u64,
    pub(super) attribute_name: &'parse str,
    pub(super) value: &'parse str,
    pub(super) decoding: LoadoutXmlDecoding,
}

impl XmlLoadoutAttributeSite<'_> {
    /// Borrows the genuine original read operation of this parse.
    #[must_use]
    pub const fn observation(&self) -> &LoadoutFileObservation {
        self.observation
    }

    /// Returns the original decoder's node index.
    #[must_use]
    pub const fn node_index(&self) -> u32 {
        self.node_index
    }

    /// Returns the winning position in the original decoder's attribute list.
    #[must_use]
    pub const fn attribute_index(&self) -> u64 {
        self.attribute_index
    }

    /// Borrows the actual winning attribute name.
    #[must_use]
    pub const fn attribute_name(&self) -> &str {
        self.attribute_name
    }

    /// Borrows the decoded original attribute before its text copy.
    #[must_use]
    pub const fn value(&self) -> &str {
        self.value
    }

    /// Returns the actual XML decoding operation.
    #[must_use]
    pub const fn decoding(&self) -> LoadoutXmlDecoding {
        self.decoding
    }
}
