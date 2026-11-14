// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufWriter, Seek as _, Write};

use crate::origin_companion::{row, write_end_counts, write_sources};
use crate::{
    OriginArtifactBinding, OriginCompanionRow, OriginOutputAddress, SourceUse, SourceUses,
};

/// Stages one streamed artifact's companion with a bounded active source-use batch.
pub struct OriginCompanionSpool {
    sources: SourceUses,
    uses: BufWriter<File>,
    fields: BufWriter<File>,
    use_count: u64,
    field_count: u64,
}

impl OriginCompanionSpool {
    /// Creates temporary streams for source uses and output field addresses.
    ///
    /// # Errors
    /// Returns a temporary file creation error.
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            sources: SourceUses::with_batched_use_refs(),
            uses: BufWriter::new(tempfile::tempfile()?),
            fields: BufWriter::new(tempfile::tempfile()?),
            use_count: 0,
            field_count: 0,
        })
    }

    /// Returns the authority for registering original inputs and current-batch uses.
    #[must_use]
    pub const fn sources(&self) -> &SourceUses {
        &self.sources
    }

    /// Stages one field from original uses of the current batch.
    ///
    /// # Errors
    /// Returns an out-of-batch evidence or output error.
    pub fn push(
        &mut self,
        address: &OriginOutputAddress,
        uses: &[SourceUse],
        complete: bool,
    ) -> io::Result<()> {
        if !self.sources.accepts_use_batch(uses)? {
            return Err(io::Error::other(
                "field origin refers outside its source-use batch",
            ));
        }
        let origin = self.sources.origin(uses, complete);
        row(
            &mut self.fields,
            &OriginCompanionRow::Field {
                address,
                origin: &origin,
            },
        )?;
        self.field_count = add_count(self.field_count, 1)?;
        Ok(())
    }

    /// Stages current source-use rows and releases the memory occupied by the batch.
    ///
    /// # Errors
    /// Returns an output or evidence error.
    pub fn finish_batch(&mut self) -> io::Result<()> {
        self.use_count = add_count(
            self.use_count,
            self.sources.write_use_batch(&mut self.uses)?,
        )?;
        Ok(())
    }

    /// Flushes temporary streams before the artifact can be committed.
    ///
    /// # Errors
    /// Returns an output or evidence error.
    pub fn finish(mut self) -> io::Result<StagedOriginCompanion> {
        self.finish_batch()?;
        let sources = self.sources.freeze().map_err(io::Error::other)?;
        let uses = self
            .uses
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        let fields = self
            .fields
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        Ok(StagedOriginCompanion {
            sources,
            uses,
            fields,
            use_count: self.use_count,
            field_count: self.field_count,
        })
    }
}

/// A complete staged companion awaiting the measured artifact binding.
pub struct StagedOriginCompanion {
    sources: crate::SourceInventory,
    uses: File,
    fields: File,
    use_count: u64,
    field_count: u64,
}

impl StagedOriginCompanion {
    /// Writes the complete versioned companion to the requested sink.
    ///
    /// # Errors
    /// Returns an output or temporary file read error.
    pub fn write(
        &mut self,
        mut writer: impl Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        row(&mut writer, &OriginCompanionRow::header(binding))?;
        write_sources(&mut writer, self.sources.sources())?;
        self.uses.rewind()?;
        io::copy(&mut self.uses, &mut writer)?;
        self.fields.rewind()?;
        io::copy(&mut self.fields, &mut writer)?;
        write_end_counts(
            &mut writer,
            self.sources.sources().len() as u64,
            self.use_count,
            self.field_count,
        )
    }
}

fn add_count(current: u64, additional: u64) -> io::Result<u64> {
    current
        .checked_add(additional)
        .ok_or_else(|| io::Error::other("origin row count exceeds u64"))
}
