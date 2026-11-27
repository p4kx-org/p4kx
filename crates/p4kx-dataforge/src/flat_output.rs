// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use super::observation::{FlatColumn, FlatColumnSpan};

pub(super) struct Cells<W, const OBSERVED: bool> {
    out: W,
    position: u64,
    spans: Vec<FlatColumnSpan>,
}

impl<W: Write, const OBSERVED: bool> Cells<W, OBSERVED> {
    pub(super) const fn new(out: W) -> Self {
        Self {
            out,
            position: 0,
            spans: Vec::new(),
        }
    }

    pub(super) fn cell(
        &mut self,
        column: FlatColumn,
        write: impl FnOnce(&mut Self) -> io::Result<()>,
    ) -> io::Result<()> {
        let start = self.position;
        write(self)?;
        if OBSERVED {
            self.spans.push(FlatColumnSpan {
                column,
                start,
                end: self.position,
            });
        }
        Ok(())
    }

    pub(super) fn prefix(&mut self, bytes: &[u8], columns: &[FlatColumnSpan]) -> io::Result<()> {
        let offset = self.position;
        self.write_all(bytes)?;
        if OBSERVED {
            self.spans.clear();
            self.spans.extend(columns.iter().map(|span| FlatColumnSpan {
                column: span.column,
                start: offset + span.start,
                end: offset + span.end,
            }));
        }
        Ok(())
    }

    pub(super) const fn spans(&self) -> &[FlatColumnSpan] {
        self.spans.as_slice()
    }

    pub(super) fn into_spans(self) -> Vec<FlatColumnSpan> {
        self.spans
    }
}

impl<W: Write, const OBSERVED: bool> Write for Cells<W, OBSERVED> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.out.write(bytes)?;
        if OBSERVED {
            self.position = self
                .position
                .checked_add(written as u64)
                .ok_or_else(|| io::Error::other("flat output exceeds the addressed byte extent"))?;
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}
