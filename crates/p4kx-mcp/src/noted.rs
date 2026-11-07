// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_export::ArchiveKeyUse;
use rmcp::ErrorData;
use rmcp::handler::server::tool::IntoCallToolResult;
use rmcp::model::{CallToolResponse, CallToolResult, ContentBlock};
use serde::Serialize;

use crate::key_report;

thread_local! {
    static PENDING: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Forgets the notice and the Archives of an earlier call on this thread.
pub(crate) fn clear() {
    PENDING.with_borrow_mut(Option::take);
    key_report::clear();
}

/// Remembers `notice` for the result of the call running on this thread.
pub(crate) fn remember(notice: Option<&str>) {
    if let Some(notice) = notice {
        PENDING.with_borrow_mut(|pending| *pending = Some(notice.to_owned()));
    }
}

/// Takes what the call that ran on this thread adds to its result.
pub(crate) fn take() -> Notes {
    Notes {
        archive: PENDING.with_borrow_mut(Option::take),
        key: key_report::take(),
    }
}

/// What a result carries besides its value: the notice of the Archive it was read from, if the
/// Archive has one, and how the Archive key served the call, if encrypted Entries stayed unread or
/// a key was used.
#[derive(Debug, Default)]
pub(crate) struct Notes {
    pub(crate) archive: Option<String>,
    pub(crate) key: Option<ArchiveKeyUse>,
}

impl Notes {
    /// Adds the notes to `result`: the Archive notice and the key note as text content blocks, the
    /// key use as `_meta`.
    pub(crate) fn apply(self, result: &mut CallToolResult) {
        result.content.extend(self.archive.map(ContentBlock::text));
        if let Some(key) = self.key {
            result
                .content
                .extend(key_report::text(key).map(ContentBlock::text));
            result.meta = Some(key_report::meta(key));
        }
    }
}

/// A structured result with the notes of the call.
///
/// The notes are more text content blocks; the structured content stays the value itself. A tool
/// returning it names its output schema with `output_schema`.
pub(crate) struct Noted<T> {
    pub(crate) value: T,
    pub(crate) notes: Notes,
}

impl<T: Serialize + 'static> IntoCallToolResult for Noted<T> {
    fn into_call_tool_result(self) -> Result<CallToolResponse, ErrorData> {
        let value = serde_json::to_value(self.value).map_err(|error| {
            ErrorData::internal_error(format!("Failed to serialize the result: {error}"), None)
        })?;
        let mut result = CallToolResult::structured(value);
        self.notes.apply(&mut result);
        Ok(result.into())
    }
}
