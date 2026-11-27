// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use rmcp::ErrorData;
use rmcp::handler::server::tool::IntoCallToolResult;
use rmcp::model::{CallToolResponse, CallToolResult, ContentBlock};
use serde::Serialize;

use crate::noted::Notes;

/// A structured result with a PNG image as an image content block, and the notes of the call.
pub(crate) struct Pictured<T> {
    pub(crate) value: T,
    pub(crate) png: Vec<u8>,
    pub(crate) notes: Notes,
}

/// A structured result with a WAV as an audio content block, and the notes of the call.
pub(crate) struct Sounded<T> {
    pub(crate) value: T,
    pub(crate) wav: Vec<u8>,
    pub(crate) notes: Notes,
}

fn attach(
    value: impl Serialize,
    block: ContentBlock,
    notes: Notes,
) -> Result<CallToolResponse, ErrorData> {
    let value = serde_json::to_value(value).map_err(|error| {
        ErrorData::internal_error(format!("Failed to serialize the result: {error}"), None)
    })?;
    let mut result = CallToolResult::structured(value);
    result.content.push(block);
    notes.apply(&mut result);
    Ok(result.into())
}

impl<T: Serialize + 'static> IntoCallToolResult for Pictured<T> {
    fn into_call_tool_result(self) -> Result<CallToolResponse, ErrorData> {
        let block = ContentBlock::image(STANDARD.encode(&self.png), "image/png");
        attach(self.value, block, self.notes)
    }
}

impl<T: Serialize + 'static> IntoCallToolResult for Sounded<T> {
    fn into_call_tool_result(self) -> Result<CallToolResponse, ErrorData> {
        let block = ContentBlock::audio(STANDARD.encode(&self.wav), "audio/wav");
        attach(self.value, block, self.notes)
    }
}
