// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{
    SourceAttributePosition, SourceContent, SourceContentPosition, SourceElementPosition,
    SourceSpanError, SourceText,
};

use crate::audio_control_payload::{AtlContentValue, AtlPosition};

pub(crate) fn source_content<'source>(
    content: &SourceContent<'source>,
) -> Result<(AtlPosition, AtlContentValue<'source>), SourceSpanError> {
    let (position, content) = match content {
        SourceContent::Text(text) => (
            content_position(text.position())?,
            AtlContentValue::Text {
                value: text.value(),
                lexeme: lexeme(text)?,
            },
        ),
        SourceContent::Comment(text) => (
            content_position(text.position())?,
            AtlContentValue::Comment {
                value: text.value(),
                lexeme: lexeme(text)?,
            },
        ),
        SourceContent::ProcessingInstruction {
            target,
            value,
            position,
            ..
        } => (
            AtlPosition::text(position),
            AtlContentValue::ProcessingInstruction {
                target,
                value: *value,
            },
        ),
        _ => return Err(SourceSpanError),
    };
    Ok((position, content))
}

fn lexeme<'document>(text: &SourceText<'document>) -> Result<&'document str, SourceSpanError> {
    text.lexeme().ok_or(SourceSpanError)
}

pub(crate) const fn element_position(
    position: &SourceElementPosition,
) -> Result<AtlPosition, SourceSpanError> {
    Ok(match position {
        SourceElementPosition::Text(range) => AtlPosition::text(range),
        SourceElementPosition::CryXml(node_index) => AtlPosition::CryXmlNode {
            node_index: *node_index,
        },
        _ => return Err(SourceSpanError),
    })
}

pub(crate) const fn declaration_position(
    position: &SourceAttributePosition,
) -> Result<AtlPosition, SourceSpanError> {
    Ok(match position {
        SourceAttributePosition::Text(range) => AtlPosition::text(range),
        SourceAttributePosition::CryXml {
            node_index,
            attribute_index,
            ..
        } => AtlPosition::CryXmlAttribute {
            node_index: *node_index,
            attribute_index: *attribute_index,
        },
        _ => return Err(SourceSpanError),
    })
}

pub(crate) const fn content_position(
    position: &SourceContentPosition,
) -> Result<AtlPosition, SourceSpanError> {
    Ok(match position {
        SourceContentPosition::Text(range) => AtlPosition::text(range),
        SourceContentPosition::CryXml(node_index) => AtlPosition::CryXmlContent {
            node_index: *node_index,
        },
        _ => return Err(SourceSpanError),
    })
}
