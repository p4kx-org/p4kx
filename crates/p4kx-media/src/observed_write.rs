// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::checkpoint::{Checkpoint, IO_BLOCK_BYTES};

pub(crate) struct ObservedWrite<'a, 'watch, W> {
    writer: W,
    checkpoint: &'a Checkpoint<'watch>,
    until_poll: usize,
}

impl<'a, 'watch, W> ObservedWrite<'a, 'watch, W> {
    pub(crate) const fn new(writer: W, checkpoint: &'a Checkpoint<'watch>) -> Self {
        Self {
            writer,
            checkpoint,
            until_poll: 0,
        }
    }
}

impl<W: Write> Write for ObservedWrite<'_, '_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.checkpoint.cancel.is_cancelled() || self.until_poll == 0 {
            self.checkpoint.check().map_err(io::Error::other)?;
            self.until_poll = IO_BLOCK_BYTES;
        }
        let prefix = bytes
            .get(..bytes.len().min(self.until_poll))
            .ok_or_else(|| io::Error::other("invalid guarded write prefix"))?;
        let written = self.writer.write(prefix)?;
        self.until_poll = self.until_poll.saturating_sub(written);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
#[path = "observed_write_tests.rs"]
mod tests;
