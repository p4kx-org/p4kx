// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{ContainerIdentity, Digest, Reference};

/// One reference classification and its uniquely proved physical input.
#[derive(Debug)]
#[non_exhaustive]
pub struct InputLookup<'directory> {
    /// Existing reference status without selection promotion.
    pub reference: Reference,
    /// Read token only for a complete, unique, lossless candidate.
    pub input: Option<EntryInput<'directory>>,
}

/// A source-bound proof that identifies one physical Entry without opening it.
#[derive(Debug)]
pub struct EntryInput<'directory> {
    pub(crate) root: &'directory Digest,
    pub(crate) chain: &'directory [ContainerIdentity],
    pub(crate) position: usize,
}
