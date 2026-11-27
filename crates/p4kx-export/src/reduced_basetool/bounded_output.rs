// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::io::{self, BufWriter, Write};

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::plan::{COMPRESSED_LIMIT, DECODED_LIMIT};
use crate::catalog_output::{ZSTD_LEVEL, encoder};
use crate::catalog_query_session::checkpoint;

const BUFFER_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LimitKind {
    Decoded,
    Compressed,
}

#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub(super) decoded: u64,
    pub(super) compressed: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            decoded: DECODED_LIMIT,
            compressed: COMPRESSED_LIMIT,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Sizes {
    pub(super) decoded: u64,
    pub(super) compressed: u64,
}

pub(super) fn measure(
    document: &impl Serialize,
    limit: u64,
    cancel: Option<&CancelToken>,
) -> Result<u64, ReducedError> {
    let exceeded = Cell::new(None);
    let mut out = Limited::new(io::sink(), LimitKind::Decoded, limit, &exceeded);
    let result = write_json(document, &mut out, cancel);
    if exceeded.get().is_some() {
        return Err(ReducedError::Limit {
            kind: LimitKind::Decoded,
            limit,
        });
    }
    result?;
    Ok(out.written)
}

pub(super) fn write(
    document: &impl Serialize,
    out: &mut impl Write,
    decoded: u64,
    limits: Limits,
    cancel: Option<&CancelToken>,
) -> Result<Sizes, ReducedError> {
    let exceeded = Cell::new(None);
    let packed = Limited::new(out, LimitKind::Compressed, limits.compressed, &exceeded);
    let result = encode(document, packed, decoded, limits.decoded, cancel);
    if let Some(kind) = exceeded.get() {
        let limit = match kind {
            LimitKind::Decoded => limits.decoded,
            LimitKind::Compressed => limits.compressed,
        };
        return Err(ReducedError::Limit { kind, limit });
    }
    result
}

fn encode<W: Write>(
    document: &impl Serialize,
    packed: Limited<'_, W>,
    decoded: u64,
    limit: u64,
    cancel: Option<&CancelToken>,
) -> Result<Sizes, ReducedError> {
    let exceeded = packed.exceeded;
    let encoder = encoder(packed, ZSTD_LEVEL, decoded)?;
    let mut plain = Limited::new(encoder, LimitKind::Decoded, limit, exceeded);
    write_json(document, &mut plain, cancel)?;
    if plain.written != decoded {
        return Err(io::Error::other("reduced output changed after measurement").into());
    }
    let packed = plain.inner.finish()?;
    Ok(Sizes {
        decoded,
        compressed: packed.written,
    })
}

fn write_json(
    document: &impl Serialize,
    sink: &mut impl Write,
    cancel: Option<&CancelToken>,
) -> Result<(), ReducedError> {
    checkpoint(cancel)?;
    let mut out = BufWriter::with_capacity(BUFFER_BYTES, sink);
    let result = (|| -> Result<(), ReducedError> {
        serde_json::to_writer(&mut out, document)?;
        out.write_all(b"\n")?;
        out.flush()?;
        Ok(())
    })();
    checkpoint(cancel)?;
    result
}

struct Limited<'scope, W> {
    inner: W,
    kind: LimitKind,
    limit: u64,
    exceeded: &'scope Cell<Option<LimitKind>>,
    written: u64,
}

impl<'scope, W> Limited<'scope, W> {
    const fn new(
        inner: W,
        kind: LimitKind,
        limit: u64,
        exceeded: &'scope Cell<Option<LimitKind>>,
    ) -> Self {
        Self {
            inner,
            kind,
            limit,
            exceeded,
            written: 0,
        }
    }
}

impl<W: Write> Write for Limited<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let requested = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if requested > self.limit - self.written {
            self.exceeded.set(Some(self.kind));
            return Err(io::Error::other("reduced catalog output limit reached"));
        }
        let count = self.inner.write(bytes)?;
        self.written += u64::try_from(count).map_err(io::Error::other)?;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
