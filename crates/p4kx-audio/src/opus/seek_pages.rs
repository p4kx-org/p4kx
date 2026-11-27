// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use super::OpusError;
use super::ogg_read::{self, HEADER_LEN, PageSource};
use crate::WemReadError;
use crate::read_at::SeekSource;

pub(super) struct SeekPage<'a, R> {
    input: &'a mut SeekSource<R>,
    data: Range<u64>,
    page: Vec<u8>,
}

impl<'a, R> SeekPage<'a, R> {
    pub(super) const fn new(input: &'a mut SeekSource<R>, data: Range<u64>) -> Self {
        Self {
            input,
            data,
            page: Vec::new(),
        }
    }
}

impl<R: Read + Seek> PageSource for SeekPage<'_, R> {
    type Error = WemReadError;

    fn length(&self) -> u64 {
        self.data.end - self.data.start
    }

    fn prefix(&mut self, offset: u64) -> Result<[u8; HEADER_LEN], WemReadError> {
        if self.length().saturating_sub(offset) < HEADER_LEN as u64 {
            return Err(OpusError::InvalidOgg("a page header is truncated").into());
        }
        let mut prefix = [0; HEADER_LEN];
        self.input
            .read_io_at(self.data.start + offset, &mut prefix)?;
        Ok(prefix)
    }

    fn load(
        &mut self,
        offset: u64,
        prefix: [u8; HEADER_LEN],
        segments: usize,
    ) -> Result<(), WemReadError> {
        let admitted = self.length().saturating_sub(offset);
        let prefix_len = HEADER_LEN + segments;
        let lacing_error = OpusError::InvalidOgg("a page's lacing is truncated");
        if admitted < prefix_len as u64 {
            return Err(lacing_error.into());
        }
        self.page.clear();
        self.page.extend_from_slice(&prefix);
        self.page.resize(prefix_len, 0);
        let lacing = self.page.get_mut(HEADER_LEN..).ok_or(lacing_error)?;
        self.input
            .read_io_at(self.data.start + offset + HEADER_LEN as u64, lacing)?;
        let page_len = ogg_read::page_len(lacing);
        let body_error = OpusError::InvalidOgg("a page body is truncated");
        if admitted < page_len as u64 {
            return Err(body_error.into());
        }
        self.page.resize(page_len, 0);
        let body = self.page.get_mut(prefix_len..).ok_or(body_error)?;
        self.input
            .read_io_at(self.data.start + offset + prefix_len as u64, body)?;
        Ok(())
    }

    fn page(&self) -> &[u8] {
        &self.page
    }
}
