// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::metadata_read_error::MetadataReadError;
use crate::metadata_span::MetadataSpan;
use crate::metadata_view::MetadataView;

/// The largest caller-owned buffer accepted by one metadata read.
pub const MAX_METADATA_CHUNK: usize = 64 * 1024;

/// Work charged to the shared caller-owned budget before access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataCost {
    read_bytes: u64,
    work_items: u64,
}

impl MetadataCost {
    /// Returns the bytes requested by the actual source operation.
    #[must_use]
    pub const fn read_bytes(self) -> u64 {
        self.read_bytes
    }

    /// Returns the bounded iterator or read operations requested.
    #[must_use]
    pub const fn work_items(self) -> u64 {
        self.work_items
    }
}

/// Bounded source access borrowing one export-wide budget adapter and cancellation flag.
pub struct MetadataAccess<'source, 'budget> {
    view: MetadataView<'source>,
    charge: &'budget mut dyn FnMut(MetadataCost) -> io::Result<()>,
    cancelled: &'budget AtomicBool,
}

impl<'source, 'budget> MetadataAccess<'source, 'budget> {
    pub(crate) fn new(
        view: MetadataView<'source>,
        charge: &'budget mut dyn FnMut(MetadataCost) -> io::Result<()>,
        cancelled: &'budget AtomicBool,
    ) -> Self {
        Self {
            view,
            charge,
            cancelled,
        }
    }

    /// Fills a caller-owned buffer from a bound span without allocating payload storage.
    ///
    /// # Errors
    /// Rejects foreign spans, oversized chunks, bounds, cancellation, budget or source failures.
    pub fn read_into(
        &mut self,
        span: MetadataSpan<'source>,
        relative_offset: u64,
        buffer: &mut [u8],
    ) -> Result<(), MetadataReadError> {
        if !span.belongs_to(self.view.source()) {
            return Err(MetadataReadError::ForeignSource);
        }
        if buffer.len() > MAX_METADATA_CHUNK {
            return Err(MetadataReadError::ChunkLimit {
                maximum: MAX_METADATA_CHUNK,
            });
        }
        let count = u64::try_from(buffer.len()).map_err(|_| MetadataReadError::OutsideSpan)?;
        if relative_offset
            .checked_add(count)
            .is_none_or(|end| end > span.range().len())
        {
            return Err(MetadataReadError::OutsideSpan);
        }
        let offset = span
            .range()
            .offset()
            .checked_add(relative_offset)
            .ok_or(MetadataReadError::OutsideSpan)?;
        self.charge(count, 1)?;
        self.view
            .source()
            .read_into(offset, buffer)
            .map_err(|source| MetadataReadError::Read { offset, source })?;
        self.check_cancelled()
    }

    pub(crate) fn charge(&mut self, bytes: u64, items: u64) -> Result<(), MetadataReadError> {
        self.check_cancelled()?;
        (self.charge)(MetadataCost {
            read_bytes: bytes,
            work_items: items,
        })
        .map_err(MetadataReadError::Budget)?;
        self.check_cancelled()
    }

    pub(crate) fn check_cancelled(&self) -> Result<(), MetadataReadError> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err(MetadataReadError::Cancelled)
        } else {
            Ok(())
        }
    }

    pub(crate) const fn view(&self) -> MetadataView<'source> {
        self.view
    }
}
