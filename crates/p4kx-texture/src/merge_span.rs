// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Write};
use std::ops::Range;

use crate::merge::{self, SiblingReader};
use crate::{DdsHeader, MergeReport, TextureError};

/// The original header or one emitted mip surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MergeSpanSite {
    /// Bytes copied from the beginning of the DDS header input.
    Header,
    /// One surface in a mip level, addressed in original DDS order.
    Mip {
        /// Mip level, zero for the largest.
        level: u32,
        /// Original surface ordinal.
        surface: u64,
        /// Sibling part number, or `None` for bytes from the header input.
        part: Option<u32>,
    },
}

/// A completed original DDS input range and its emitted output range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct MergeSpan {
    /// The logical DDS datum.
    pub site: MergeSpanSite,
    /// Byte offset within the decoded header or sibling input.
    pub input_start: u64,
    /// Byte offset within the merged DDS.
    pub output_start: u64,
    /// Number of bytes copied to that output range.
    pub byte_length: u64,
}

/// Merges a DDS and observes its header and emitted mip ranges in output order.
///
/// A successful return confirms all observed spans. On error the destination and observations
/// may contain a prefix and must not be published as complete.
///
/// # Errors
/// Returns ordinary merge errors or a failed observer as a write error.
pub fn merge_with_spans(
    header: impl Read,
    header_size: u64,
    siblings: &(impl SiblingReader + ?Sized),
    out: &mut (impl Write + ?Sized),
    mut observe: impl FnMut(MergeSpan) -> io::Result<()>,
) -> Result<MergeReport, TextureError> {
    let mut observer = SpanObserver::new(&mut observe);
    merge::merge_impl(header, header_size, siblings, out, &mut observer)
}

#[derive(Default)]
pub(crate) struct SpanObserver<'a> {
    observe: Option<&'a mut dyn FnMut(MergeSpan) -> io::Result<()>>,
    output_next: u64,
    header_tail_next: u64,
}

impl<'a> SpanObserver<'a> {
    pub(crate) fn new(observe: &'a mut dyn FnMut(MergeSpan) -> io::Result<()>) -> Self {
        Self {
            observe: Some(observe),
            ..Self::default()
        }
    }

    pub(crate) fn header(&mut self, length: u64) -> Result<(), TextureError> {
        self.header_tail_next = length;
        self.emit(MergeSpanSite::Header, 0, length)
    }

    pub(crate) fn part(
        &mut self,
        level: u32,
        surface: u64,
        index: u32,
        start: u64,
        length: u64,
    ) -> Result<(), TextureError> {
        self.emit(
            MergeSpanSite::Mip {
                level,
                surface,
                part: Some(index),
            },
            start,
            length,
        )
    }

    /// Subdivides a contiguous header tail that the merger already copied.
    pub(crate) fn tail(
        &mut self,
        dds: &DdsHeader,
        first_level: u32,
        surfaces: Range<u64>,
        length: u64,
    ) -> Result<(), TextureError> {
        if self.observe.is_none() {
            return Ok(());
        }
        let end = self
            .header_tail_next
            .checked_add(length)
            .ok_or_else(invalid_span)?;
        for surface in surfaces {
            for level in first_level..dds.mip_count() {
                let size = dds.surface_size(level).ok_or_else(invalid_span)?;
                self.emit(
                    MergeSpanSite::Mip {
                        level,
                        surface,
                        part: None,
                    },
                    self.header_tail_next,
                    size,
                )?;
                self.header_tail_next = self
                    .header_tail_next
                    .checked_add(size)
                    .ok_or_else(invalid_span)?;
            }
        }
        if self.header_tail_next != end {
            return Err(invalid_span());
        }
        Ok(())
    }

    pub(crate) fn finish(&self, report: MergeReport) -> Result<MergeReport, TextureError> {
        if self.observe.is_some() && self.output_next != report.size {
            return Err(invalid_span());
        }
        Ok(report)
    }

    fn emit(
        &mut self,
        site: MergeSpanSite,
        input_start: u64,
        byte_length: u64,
    ) -> Result<(), TextureError> {
        if let Some(observe) = &mut self.observe {
            let output_start = self.output_next;
            let next = output_start
                .checked_add(byte_length)
                .ok_or_else(invalid_span)?;
            observe(MergeSpan {
                site,
                input_start,
                output_start,
                byte_length,
            })
            .map_err(TextureError::Write)?;
            self.output_next = next;
        }
        Ok(())
    }
}

fn invalid_span() -> TextureError {
    TextureError::Write(io::Error::other(
        "the observed DDS ranges do not match the written artifact",
    ))
}
