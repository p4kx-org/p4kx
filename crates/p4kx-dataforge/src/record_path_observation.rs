// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::{DataForge, Record, RecordText};

/// How the original path index used a Record's ordering name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordOrderingName {
    /// The original name text was read.
    Read,
    /// The name is absent or uses the authored no-string sentinel.
    Absent,
    /// The name read failed and the path index used its empty fallback.
    Unreadable,
    /// The unreadable filename excluded this Record before its name was read.
    NotConsulted,
}

/// One original Record's filename and ordering-name evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RecordPathObservation {
    /// The actual physical Record table index.
    pub record_index: u32,
    /// The original NUL-terminated filename bytes.
    pub file_name: Option<Range<u64>>,
    /// The original NUL-terminated ordering name, when readable.
    pub record_name: Option<Range<u64>>,
    /// The actual ordering-name decision, including an authored absent name.
    pub name_state: RecordOrderingName,
    /// Whether the canonical path order includes this physical header.
    pub indexed: bool,
}

impl RecordPathObservation {
    pub(crate) fn read(record: &Record<'_>) -> Self {
        let forge = record.forge;
        let file = record.stored_file_name().ok();
        let (record_name, name_state) = if file.is_some() {
            match record.stored_name() {
                Ok(RecordText::Present(text)) => {
                    (terminated_text_span(forge, text), RecordOrderingName::Read)
                }
                Ok(RecordText::Absent | RecordText::Null) => (None, RecordOrderingName::Absent),
                Err(_) => (None, RecordOrderingName::Unreadable),
            }
        } else {
            (None, RecordOrderingName::NotConsulted)
        };
        Self {
            record_index: record.index(),
            file_name: file.and_then(|text| terminated_text_span(forge, text)),
            record_name,
            name_state,
            indexed: file.is_some(),
        }
    }
}

fn terminated_text_span(forge: &DataForge<'_>, text: &str) -> Option<Range<u64>> {
    let span = forge.original_text_span(text)?;
    if text.is_empty() {
        return Some(span);
    }

    let terminator = usize::try_from(span.end).ok()?;
    if forge.bytes.get(terminator) != Some(&0) {
        return None;
    }
    Some(span.start..span.end.checked_add(1)?)
}

impl DataForge<'_> {
    /// Reports the physical Record path census and returns the canonical path order.
    ///
    /// The observer also receives headers omitted from path order due to unreadable filenames.
    /// It reads only the original Record header and string tables.
    pub fn path_order_observed(&self, mut observe: impl FnMut(RecordPathObservation)) -> &[u32] {
        let order = self.path_order();
        for record in self.records() {
            observe(RecordPathObservation::read(&record));
        }
        order
    }
}
