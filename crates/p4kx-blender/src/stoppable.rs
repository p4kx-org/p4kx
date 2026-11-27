// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::{PackageError, PackageWriteContext};

/// Stops writes before passing another fragment to the actual sink.
pub(crate) struct Stoppable<'a, W> {
    inner: W,
    context: &'a PackageWriteContext<'a>,
}

impl<'a, W> Stoppable<'a, W> {
    pub(crate) const fn new(inner: W, context: &'a PackageWriteContext<'a>) -> Self {
        Self { inner, context }
    }

    pub(crate) fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for Stoppable<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.context.check().map_err(io::Error::other)?;
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(crate) fn is_cancelled(error: &io::Error) -> bool {
    matches!(
        error
            .get_ref()
            .and_then(|error| error.downcast_ref::<PackageError>()),
        Some(PackageError::Cancelled)
    )
}
