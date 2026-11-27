// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_dataforge::{DataForge, PositionedRecord};

use crate::{DataForgeEntry, DataForgeOrigin, SourceEvidenceError};

/// Admits only Records from the exact DataForge opened over an observed Entry.
#[derive(Clone, Copy)]
pub struct RecordStreamInput<'input, 'database> {
    origin: &'input DataForgeOrigin,
    forge: &'database DataForge<'database>,
}

impl<'input, 'database> RecordStreamInput<'input, 'database> {
    /// Binds the decoded database to the original Entry that was opened.
    ///
    /// # Errors
    /// Returns a foreign evidence error if the original bytes differ by identity.
    pub fn new(
        input: &'input DataForgeEntry,
        forge: &'database DataForge<'database>,
    ) -> io::Result<Self> {
        let origin = input.origin_of(forge).ok_or_else(foreign)?;
        Ok(Self { origin, forge })
    }

    pub(crate) const fn origin(self) -> &'input DataForgeOrigin {
        self.origin
    }

    pub(super) const fn forge(self) -> &'database DataForge<'database> {
        self.forge
    }

    pub(crate) fn check(self, record: &PositionedRecord<'_>) -> io::Result<()> {
        if self
            .forge
            .record(record.tree().record_index())
            .is_some_and(|original| record.tree().is_from(&original))
        {
            Ok(())
        } else {
            Err(foreign())
        }
    }
}

pub(crate) fn foreign() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        SourceEvidenceError::ForeignEvidence,
    )
}
