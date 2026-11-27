// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

pub(crate) struct Positioned<W> {
    out: W,
    position: u64,
}

impl<W> Positioned<W> {
    pub(crate) const fn new(out: W) -> Self {
        Self { out, position: 0 }
    }

    pub(crate) const fn position(&self) -> u64 {
        self.position
    }
}

impl<W: Write> Write for Positioned<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.out.write(bytes)?;
        self.position = self
            .position
            .checked_add(written as u64)
            .ok_or_else(|| io::Error::other("query text position exceeds u64"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}
