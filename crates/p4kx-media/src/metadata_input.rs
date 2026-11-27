// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use p4kx_extraction::{MAX_METADATA_CHUNK, MetadataAccess, MetadataSpan};

use crate::MediaError;

pub(crate) struct MetadataInput<'source, 'budget, 'access> {
    access: &'access mut MetadataAccess<'source, 'budget>,
    span: MetadataSpan<'source>,
    position: u64,
}

impl<'source, 'budget, 'access> MetadataInput<'source, 'budget, 'access> {
    pub(crate) const fn new(
        access: &'access mut MetadataAccess<'source, 'budget>,
        span: MetadataSpan<'source>,
    ) -> Self {
        Self {
            access,
            span,
            position: 0,
        }
    }
}

impl Read for MetadataInput<'_, '_, '_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let remaining = self.span.range().len() - self.position;
        let count = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(output.len())
            .min(MAX_METADATA_CHUNK);
        if count == 0 {
            return Ok(0);
        }
        let buffer = output
            .get_mut(..count)
            .ok_or_else(|| io::Error::other("invalid metadata input capacity"))?;
        self.access
            .read_into(self.span, self.position, buffer)
            .map_err(|cause| io::Error::other(MediaError::from(cause)))?;
        self.position += u64::try_from(count)
            .map_err(|_| io::Error::other("metadata input count exceeds source width"))?;
        Ok(count)
    }
}
