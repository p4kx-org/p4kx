// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::{MediaError, MediaWriter, SourceContext};

const COUNT_LENGTH: usize = size_of::<u32>();
const FIRST_PATH: usize = 12;
pub(super) const PATH_LENGTH: usize = 256;
const ALIGNMENT: usize = size_of::<u32>();
const ROOT: &[u8] = b"Animations";

pub(super) struct DirectionalPaths<'a> {
    bytes: &'a [u8],
    start: u64,
}

pub(super) struct DirectionalPath<'a> {
    pub(super) text: &'a str,
    pub(super) range: Range<u64>,
}

impl<'a> DirectionalPaths<'a> {
    pub(super) fn new(bytes: &'a [u8], start: u64) -> Option<Self> {
        start.checked_add(bytes.len() as u64)?;
        let _: &[u8; COUNT_LENGTH] = bytes.first_chunk()?;
        Some(Self { bytes, start })
    }

    pub(super) fn declared(&self) -> Option<usize> {
        let count = u32::from_le_bytes(*self.bytes.first_chunk()?);
        usize::try_from(count).ok()
    }

    pub(super) fn visit(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
        mut observe: impl FnMut(DirectionalPath<'a>, &mut MediaWriter<'_>) -> Result<(), MediaError>,
    ) -> Result<(), MediaError> {
        const BLOCK_BYTES: usize = 64 << 10;
        let budget = &context.directory.input_state()?.budget;
        for block in (FIRST_PATH..self.bytes.len()).step_by(BLOCK_BYTES) {
            let end = block.saturating_add(BLOCK_BYTES).min(self.bytes.len());
            let steps = (end - block).div_ceil(ALIGNMENT) as u64;
            budget.metadata(0, steps, writer.limits(), writer.checkpoint)?;
            for offset in (block..end).step_by(ALIGNMENT) {
                let Some(path) = self.path_at(offset) else {
                    continue;
                };
                observe(path, writer)?;
            }
        }
        Ok(())
    }

    pub(super) fn begins_with_path(&self) -> bool {
        self.path_at(FIRST_PATH).is_some()
    }

    fn path_at(&self, offset: usize) -> Option<DirectionalPath<'a>> {
        let tail = self.bytes.get(offset..)?;
        let field = tail.first_chunk::<PATH_LENGTH>()?;
        let text = path_text(field)?;
        let from = self.start + offset as u64;
        Some(DirectionalPath {
            text,
            range: from..from + PATH_LENGTH as u64,
        })
    }
}

pub(super) fn path_text(field: &[u8; PATH_LENGTH]) -> Option<&str> {
    let (root, tail) = field.split_first_chunk::<{ ROOT.len() }>()?;
    if !root.eq_ignore_ascii_case(ROOT) || !matches!(tail.first(), Some(b'/' | b'\\')) {
        return None;
    }
    let nul = field.iter().position(|byte| *byte == 0)?;
    let (text, padding) = field.split_at_checked(nul)?;
    if !padding.iter().all(|byte| *byte == 0) {
        return None;
    }
    let text = std::str::from_utf8(text).ok()?;
    let (path, extension) = text.rsplit_once('.')?;
    (path.len() > ROOT.len() + 1
        && extension.eq_ignore_ascii_case("aim")
        && !path.chars().any(char::is_control))
    .then_some(text)
}
